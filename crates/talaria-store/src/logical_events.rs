// crates/talaria-store/src/logical_events.rs
//! Stable identity of a historical fact, independent of title, date, place, and type.
//! Versions stay on `canonical_events`. A shared title or a similar occurrence key
//! does not make two rows the same fact.

use sqlx::{Connection, PgPool};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

pub const ISSUE_ONE_WAY: &str = "one_way_supersession";
/// Inactive row points at an active row that does not point back.
/// One predecessor, same person, same type. Reported, not merged.
pub const ISSUE_FORWARD_ONLY: &str = "forward_only_version";
pub const ISSUE_DANGLING: &str = "dangling_supersession";
pub const ISSUE_CYCLE: &str = "cycle";
pub const ISSUE_CONFLICT: &str = "conflicting_logical_ids";
pub const ISSUE_MULTIPLE_ACTIVE: &str = "multiple_active_versions";
pub const ISSUE_CROSS_ENTITY: &str = "cross_entity_chain";
pub const ISSUE_INACTIVE_UNLINKED: &str = "inactive_unlinked";

const BACKFILL_LOCK: i64 = 53_001_001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalIdentityIssue {
    pub kind: &'static str,
    pub event_id: Uuid,
    pub other_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalIdentityBackfill {
    pub assigned_events: i64,
    pub created_identities: i64,
    pub issues: Vec<LogicalIdentityIssue>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IdentityVersion {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub event_type: String,
    pub supersedes: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub logical_event_id: Option<Uuid>,
    pub occurrence_key: Option<String>,
    pub is_active: bool,
}

struct IdentityGroup {
    event_ids: Vec<Uuid>,
    existing_logical_event_id: Option<Uuid>,
    origin_occurrence_key: Option<String>,
}

struct IdentityPlan {
    groups: Vec<IdentityGroup>,
    issues: Vec<LogicalIdentityIssue>,
    parked_restorations: usize,
    version_chains: usize,
    coherent_chains: usize,
    active_receiving_identity: usize,
}

pub async fn create_logical_event<'e, E>(
    executor: E,
    origin_occurrence_key: Option<&str>,
) -> anyhow::Result<Uuid>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let id = sqlx::query_scalar(
        "INSERT INTO logical_events (origin_occurrence_key) VALUES ($1) RETURNING id",
    )
    .bind(origin_occurrence_key)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

pub async fn logical_event_id_of(pool: &PgPool, event_id: Uuid) -> anyhow::Result<Option<Uuid>> {
    let id = sqlx::query_scalar("SELECT logical_event_id FROM canonical_events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(pool)
        .await?;
    Ok(id.flatten())
}

/// Assign one logical identity to each active event, and share it only along a
/// demonstrated supersession. An inherited `superseded_by` without a matching
/// `supersedes` stays unresolved: the archive keeps `logical_event_id` NULL.
/// Rows already assigned are left in place. A cycle or two identities on one
/// demonstrated chain are reported and not rewritten.
pub async fn backfill_logical_identities(pool: &PgPool) -> anyhow::Result<LogicalIdentityBackfill> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(BACKFILL_LOCK)
        .execute(&mut *conn)
        .await?;
    let result = apply_backfill(&mut conn).await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(BACKFILL_LOCK)
        .execute(&mut *conn)
        .await?;
    result
}

async fn apply_backfill(conn: &mut sqlx::PgConnection) -> anyhow::Result<LogicalIdentityBackfill> {
    let rows = load_identity_versions_on(conn).await?;
    let plan = plan_logical_identities(&rows);
    let mut assigned_events = 0i64;
    let mut created_identities = 0i64;
    for group in plan.groups {
        let mut tx = conn.begin().await?;
        let logical_id = match group.existing_logical_event_id {
            Some(id) => id,
            None => {
                created_identities += 1;
                create_logical_event(&mut *tx, group.origin_occurrence_key.as_deref()).await?
            }
        };
        let updated = sqlx::query(
            r#"
            UPDATE canonical_events
            SET logical_event_id = $1
            WHERE id = ANY($2) AND logical_event_id IS NULL
            "#,
        )
        .bind(logical_id)
        .bind(&group.event_ids)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        assigned_events += updated.rows_affected() as i64;
    }
    Ok(LogicalIdentityBackfill {
        assigned_events,
        created_identities,
        issues: plan.issues,
    })
}

fn plan_logical_identities(rows: &[IdentityVersion]) -> IdentityPlan {
    let by_id: HashMap<Uuid, &IdentityVersion> = rows.iter().map(|row| (row.id, row)).collect();
    let mut successor: HashMap<Uuid, Uuid> = HashMap::new();
    let mut issues: BTreeSet<(&'static str, Uuid, Option<Uuid>)> = BTreeSet::new();
    let mut pointed_at: HashMap<Uuid, usize> = HashMap::new();
    for row in rows {
        if let Some(next) = row.superseded_by {
            *pointed_at.entry(next).or_default() += 1;
        }
    }

    for row in rows {
        if let Some(next) = row.superseded_by {
            if !by_id.contains_key(&next) {
                issues.insert((ISSUE_DANGLING, row.id, Some(next)));
            } else if by_id[&next].supersedes == Some(row.id) {
                successor.insert(row.id, next);
            }
        }
        if let Some(previous) = row.supersedes {
            if !by_id.contains_key(&previous) {
                issues.insert((ISSUE_DANGLING, row.id, Some(previous)));
            }
        }
    }

    let mut parent: HashMap<Uuid, Uuid> = rows.iter().map(|row| (row.id, row.id)).collect();
    for (from, to) in successor.iter() {
        union(&mut parent, *from, *to);
    }

    let mut cyclic_members = HashSet::new();
    for members in component_members(&parent, rows) {
        if chain_is_cyclic(&members, &successor) {
            cyclic_members.extend(members);
        }
    }

    let mut parked = HashSet::new();
    for row in rows {
        if parked_restoration(row, &by_id, &successor, &mut parent, &cyclic_members) {
            parked.insert(row.id);
            if let Some(restored) = row.superseded_by {
                union(&mut parent, row.id, restored);
            }
        }
    }

    for row in rows {
        if parked.contains(&row.id) {
            continue;
        }
        if !row.is_active && row.supersedes.is_none() && row.superseded_by.is_none() {
            issues.insert((ISSUE_INACTIVE_UNLINKED, row.id, None));
        }
        if let Some(next) = row.superseded_by {
            if by_id.contains_key(&next) && successor.get(&row.id) != Some(&next) {
                let kind = if forward_only_version(
                    row,
                    &by_id[&next],
                    pointed_at.get(&next).copied().unwrap_or(0),
                ) {
                    ISSUE_FORWARD_ONLY
                } else {
                    ISSUE_ONE_WAY
                };
                issues.insert((kind, row.id, Some(next)));
            }
        }
        if let Some(previous) = row.supersedes {
            if by_id.contains_key(&previous) && successor.get(&previous) != Some(&row.id) {
                issues.insert((ISSUE_ONE_WAY, row.id, Some(previous)));
            }
        }
    }

    let mut groups = Vec::new();
    let mut version_chains = 0usize;
    let mut coherent_chains = 0usize;
    let mut active_receiving_identity = 0usize;
    for mut members in component_members(&parent, rows) {
        members.sort();
        let chained = members.len() > 1;
        if chained {
            version_chains += 1;
        }
        if members.iter().any(|id| cyclic_members.contains(id)) {
            issues.insert((ISSUE_CYCLE, members[0], None));
            continue;
        }
        let entities: HashSet<Uuid> = members.iter().map(|id| by_id[id].entity_id).collect();
        if entities.len() > 1 {
            issues.insert((ISSUE_CROSS_ENTITY, members[0], None));
            continue;
        }
        let active = members.iter().filter(|id| by_id[id].is_active).count();
        if active > 1 {
            issues.insert((ISSUE_MULTIPLE_ACTIVE, members[0], None));
            continue;
        }
        let mut known: Vec<Uuid> = members
            .iter()
            .filter_map(|id| by_id[id].logical_event_id)
            .collect();
        known.sort();
        known.dedup();
        if known.len() > 1 {
            issues.insert((ISSUE_CONFLICT, members[0], None));
            continue;
        }
        if chained {
            coherent_chains += 1;
        }
        // A title-collapse archive is its own component. It is not a version of
        // the active row, so it keeps a null identity until a review says otherwise.
        if !chained && active == 0 {
            continue;
        }
        active_receiving_identity += active;
        if members
            .iter()
            .all(|id| by_id[id].logical_event_id.is_some())
        {
            continue;
        }
        let pointed: HashSet<Uuid> = members
            .iter()
            .filter_map(|id| successor.get(id).copied())
            .filter(|next| members.binary_search(next).is_ok())
            .collect();
        let root = members.iter().find(|id| !pointed.contains(id)).copied();
        let origin = root
            .and_then(|id| by_id[&id].occurrence_key.clone())
            .or_else(|| by_id[&members[0]].occurrence_key.clone());
        groups.push(IdentityGroup {
            event_ids: members,
            existing_logical_event_id: known.first().copied(),
            origin_occurrence_key: origin,
        });
    }

    let issues = issues
        .into_iter()
        .map(|(kind, event_id, other_event_id)| LogicalIdentityIssue {
            kind,
            event_id,
            other_event_id,
        })
        .collect();
    IdentityPlan {
        groups,
        issues,
        parked_restorations: parked.len(),
        version_chains,
        coherent_chains,
        active_receiving_identity,
    }
}

fn component_members(parent: &HashMap<Uuid, Uuid>, rows: &[IdentityVersion]) -> Vec<Vec<Uuid>> {
    let mut parent = parent.clone();
    let mut members_of: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for row in rows {
        members_of
            .entry(find(&mut parent, row.id))
            .or_default()
            .push(row.id);
    }
    members_of.into_values().collect()
}

/// A reverted correction points at the restored version in both directions of its
/// own row, while the restored version does not point back. That is history, not
/// a broken chain.
fn forward_only_version(
    row: &IdentityVersion,
    target: &IdentityVersion,
    predecessors: usize,
) -> bool {
    !row.is_active
        && row.supersedes.is_none()
        && target.is_active
        && target.supersedes.is_none()
        && target.entity_id == row.entity_id
        && target.event_type == row.event_type
        && predecessors == 1
}

fn parked_restoration(
    row: &IdentityVersion,
    by_id: &HashMap<Uuid, &IdentityVersion>,
    successor: &HashMap<Uuid, Uuid>,
    parent: &mut HashMap<Uuid, Uuid>,
    cyclic_members: &HashSet<Uuid>,
) -> bool {
    if row.is_active {
        return false;
    }
    let (Some(origin), Some(restored)) = (row.supersedes, row.superseded_by) else {
        return false;
    };
    let (Some(origin_row), Some(restored_row)) = (by_id.get(&origin), by_id.get(&restored)) else {
        return false;
    };
    if origin_row.entity_id != row.entity_id || restored_row.entity_id != row.entity_id {
        return false;
    }
    if successor.get(&row.id) == Some(&restored) || cyclic_members.contains(&row.id) {
        return false;
    }
    if cyclic_members.contains(&origin) || cyclic_members.contains(&restored) {
        return false;
    }
    find(parent, origin) == find(parent, restored)
}

fn chain_is_cyclic(members: &[Uuid], successor: &HashMap<Uuid, Uuid>) -> bool {
    let member_set: HashSet<Uuid> = members.iter().copied().collect();
    for start in members {
        let mut seen = HashSet::new();
        let mut current = *start;
        loop {
            if !member_set.contains(&current) || !seen.insert(current) {
                return seen.contains(&current) && member_set.contains(&current);
            }
            match successor.get(&current).copied() {
                Some(next) if member_set.contains(&next) => current = next,
                _ => break,
            }
        }
    }
    false
}

fn find(parent: &mut HashMap<Uuid, Uuid>, mut id: Uuid) -> Uuid {
    let mut root = id;
    while parent[&root] != root {
        root = parent[&root];
    }
    while parent[&id] != root {
        let next = parent[&id];
        parent.insert(id, root);
        id = next;
    }
    root
}

fn union(parent: &mut HashMap<Uuid, Uuid>, left: Uuid, right: Uuid) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        parent.insert(right_root, left_root);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalIdentityDiagnostic {
    pub canonical_rows: usize,
    pub version_chains: usize,
    pub coherent_chains: usize,
    pub cycles: usize,
    pub one_way_links: usize,
    pub forward_only_versions: usize,
    pub multiple_active: usize,
    pub inactive_unlinked: usize,
    pub cross_entity: usize,
    pub identity_conflicts: usize,
    pub dangling_links: usize,
    pub parked_restorations: usize,
    /// Active rows that can receive an identity without joining an unresolved archive.
    pub active_receiving_identity: usize,
    /// Inherited links and inactive rows left without an identity.
    pub unresolved_archives: usize,
    pub issues: Vec<LogicalIdentityIssue>,
}

impl LogicalIdentityDiagnostic {
    /// True when a progressive backfill could write a corrupt chain.
    /// Unresolved historical links do not block it: those archives stay unassigned.
    pub fn blocks_migration(&self) -> bool {
        self.cycles
            + self.multiple_active
            + self.cross_entity
            + self.identity_conflicts
            + self.dangling_links
            > 0
    }
}

/// Read the version graph. Does not assign identities and does not update rows.
pub async fn diagnose_logical_identities(
    pool: &PgPool,
) -> anyhow::Result<LogicalIdentityDiagnostic> {
    let rows = load_identity_versions(pool).await?;
    Ok(diagnostic_from(&rows))
}

fn diagnostic_from(rows: &[IdentityVersion]) -> LogicalIdentityDiagnostic {
    let plan = plan_logical_identities(rows);
    let count = |kind: &str| {
        plan.issues
            .iter()
            .filter(|issue| issue.kind == kind)
            .count()
    };
    LogicalIdentityDiagnostic {
        canonical_rows: rows.len(),
        version_chains: plan.version_chains,
        coherent_chains: plan.coherent_chains,
        cycles: count(ISSUE_CYCLE),
        one_way_links: count(ISSUE_ONE_WAY),
        forward_only_versions: count(ISSUE_FORWARD_ONLY),
        multiple_active: count(ISSUE_MULTIPLE_ACTIVE),
        inactive_unlinked: count(ISSUE_INACTIVE_UNLINKED),
        cross_entity: count(ISSUE_CROSS_ENTITY),
        identity_conflicts: count(ISSUE_CONFLICT),
        dangling_links: count(ISSUE_DANGLING),
        parked_restorations: plan.parked_restorations,
        active_receiving_identity: plan.active_receiving_identity,
        unresolved_archives: count(ISSUE_ONE_WAY)
            + count(ISSUE_FORWARD_ONLY)
            + count(ISSUE_INACTIVE_UNLINKED),
        issues: plan.issues,
    }
}

async fn load_identity_versions_on(
    conn: &mut sqlx::PgConnection,
) -> anyhow::Result<Vec<IdentityVersion>> {
    let has_logical: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = 'canonical_events'
              AND column_name = 'logical_event_id'
        )
        "#,
    )
    .fetch_one(&mut *conn)
    .await?;
    let sql = if has_logical {
        r#"
        SELECT id, entity_id, event_type, supersedes, superseded_by, logical_event_id,
               occurrence_key, is_active
        FROM canonical_events
        "#
    } else {
        r#"
        SELECT id, entity_id, event_type, supersedes, superseded_by,
               NULL::uuid AS logical_event_id, occurrence_key, is_active
        FROM canonical_events
        "#
    };
    Ok(sqlx::query_as(sql).fetch_all(&mut *conn).await?)
}

async fn load_identity_versions(pool: &PgPool) -> anyhow::Result<Vec<IdentityVersion>> {
    let mut conn = pool.acquire().await?;
    load_identity_versions_on(&mut conn).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(
        n: u128,
        entity: u128,
        supersedes: Option<u128>,
        superseded_by: Option<u128>,
        key: &str,
        active: bool,
    ) -> IdentityVersion {
        IdentityVersion {
            id: Uuid::from_u128(n),
            entity_id: Uuid::from_u128(entity),
            event_type: "historical_fact".into(),
            supersedes: supersedes.map(Uuid::from_u128),
            superseded_by: superseded_by.map(Uuid::from_u128),
            logical_event_id: None,
            occurrence_key: Some(key.into()),
            is_active: active,
        }
    }

    fn kinds(plan: &IdentityPlan) -> Vec<&'static str> {
        plan.issues.iter().map(|issue| issue.kind).collect()
    }

    #[test]
    fn logical_identity_plan_joins_a_chain_and_refuses_uncertain_links() {
        let chain = [
            version(1, 10, None, Some(2), "lodi-year", false),
            version(2, 10, Some(1), None, "lodi-day", true),
        ];
        let plan = plan_logical_identities(&chain);
        assert!(plan.issues.is_empty());
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(
            plan.groups[0].event_ids,
            vec![Uuid::from_u128(1), Uuid::from_u128(2)]
        );
        assert_eq!(
            plan.groups[0].origin_occurrence_key.as_deref(),
            Some("lodi-year")
        );

        let similar_keys = [
            version(3, 10, None, None, "mondovi-1796", true),
            version(4, 10, None, None, "mondovi-1796-april", false),
        ];
        let plan = plan_logical_identities(&similar_keys);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].event_ids, vec![Uuid::from_u128(3)]);
        assert_eq!(kinds(&plan), vec![ISSUE_INACTIVE_UNLINKED]);

        let forward_only = [
            version(5, 10, None, Some(6), "same-title", false),
            version(6, 10, None, None, "same-title", true),
        ];
        let plan = plan_logical_identities(&forward_only);
        assert_eq!(kinds(&plan), vec![ISSUE_FORWARD_ONLY]);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].event_ids, vec![Uuid::from_u128(6)]);
        assert!(!diagnostic_from(&forward_only).blocks_migration());

        let fan_in = [
            version(41, 10, None, Some(43), "first-reading", false),
            version(42, 10, None, Some(43), "second-reading", false),
            version(43, 10, None, None, "kept", true),
        ];
        let plan = plan_logical_identities(&fan_in);
        assert_eq!(kinds(&plan), vec![ISSUE_ONE_WAY, ISSUE_ONE_WAY]);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].event_ids, vec![Uuid::from_u128(43)]);
        assert!(!diagnostic_from(&fan_in).blocks_migration());
        assert_eq!(diagnostic_from(&fan_in).unresolved_archives, 2);
        assert_eq!(diagnostic_from(&fan_in).active_receiving_identity, 1);

        let mut mismatched = [
            version(51, 10, None, Some(52), "battle-reading", false),
            version(52, 10, None, None, "office-reading", true),
        ];
        mismatched[0].event_type = "battle".into();
        mismatched[1].event_type = "office".into();
        let plan = plan_logical_identities(&mismatched);
        assert_eq!(kinds(&plan), vec![ISSUE_ONE_WAY]);

        let cycle = [
            version(7, 10, Some(8), Some(8), "cycle-a", false),
            version(8, 10, Some(7), Some(7), "cycle-b", true),
        ];
        let plan = plan_logical_identities(&cycle);
        assert_eq!(kinds(&plan), vec![ISSUE_CYCLE]);
        assert!(plan.groups.is_empty());

        let mut conflict = [
            version(1, 10, None, Some(2), "kept", false),
            version(2, 10, Some(1), None, "kept", true),
        ];
        conflict[0].logical_event_id = Some(Uuid::from_u128(100));
        conflict[1].logical_event_id = Some(Uuid::from_u128(200));
        let plan = plan_logical_identities(&conflict);
        assert_eq!(kinds(&plan), vec![ISSUE_CONFLICT]);
        assert!(plan.groups.is_empty());

        let dangling = [version(9, 10, None, Some(99), "missing-successor", true)];
        let plan = plan_logical_identities(&dangling);
        assert_eq!(kinds(&plan), vec![ISSUE_DANGLING]);
        assert_eq!(plan.groups.len(), 1);

        let crossed = [
            version(11, 10, None, Some(12), "shared", false),
            version(12, 20, Some(11), None, "shared", true),
        ];
        let plan = plan_logical_identities(&crossed);
        assert_eq!(kinds(&plan), vec![ISSUE_CROSS_ENTITY]);
        assert!(plan.groups.is_empty());

        let both_active = [
            version(13, 10, None, Some(14), "both", true),
            version(14, 10, Some(13), None, "both", true),
        ];
        let plan = plan_logical_identities(&both_active);
        assert_eq!(kinds(&plan), vec![ISSUE_MULTIPLE_ACTIVE]);
        assert!(plan.groups.is_empty());

        let restored = [
            version(21, 10, None, None, "restored", true),
            version(22, 10, Some(21), Some(21), "parked-correction", false),
        ];
        let plan = plan_logical_identities(&restored);
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.parked_restorations, 1);
        assert_eq!(plan.groups.len(), 1);
        let report = diagnostic_from(&restored);
        assert!(!report.blocks_migration());
        assert_eq!(report.coherent_chains, 1);

        let continued = [
            version(21, 10, None, Some(23), "restored", false),
            version(22, 10, Some(21), Some(21), "parked-correction", false),
            version(23, 10, Some(21), None, "later-correction", true),
        ];
        let plan = plan_logical_identities(&continued);
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.groups[0].event_ids.len(), 3);
        assert_eq!(plan.parked_restorations, 1);

        let successive = [
            version(31, 10, None, Some(32), "first", false),
            version(32, 10, Some(31), Some(33), "second", false),
            version(33, 10, Some(32), None, "third", true),
        ];
        let plan = plan_logical_identities(&successive);
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.parked_restorations, 0);
        assert_eq!(plan.groups[0].event_ids.len(), 3);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn logical_identity_survives_correction_and_backfill(pool: sqlx::PgPool) {
        use chrono::TimeZone;
        let utc = chrono::Utc;

        let napoleon = crate::ensure_subject_entity(&pool, "Q517-logical", "Napoleon Bonaparte")
            .await
            .unwrap();
        let curie = crate::ensure_subject_entity(&pool, "Q7186-logical", "Marie Curie")
            .await
            .unwrap();
        let hugo = crate::ensure_subject_entity(&pool, "Q535-logical", "Victor Hugo")
            .await
            .unwrap();
        let leonardo = crate::ensure_subject_entity(&pool, "Q762-logical", "Leonardo da Vinci")
            .await
            .unwrap();
        let aristotle = crate::ensure_subject_entity(&pool, "Q868-logical", "Aristotle")
            .await
            .unwrap();

        let lodi_quote = "Napoleon defeated the Austrian army at Lodi on 10 May 1796.";
        let lodi = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: napoleon,
                event_type: "battle".into(),
                epistemic_status: "attested".into(),
                title: "Battle of Lodi".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1796, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "exact", "precision": "year", "start": "1796"}),
                place_label: Some("Lodi".into()),
                lat: Some(45.314),
                lon: Some(9.503),
                confidence: 0.9,
                map_eligible: true,
                fingerprint: "logical-lodi".into(),
                occurrence_key: "logical-lodi-year".into(),
                occurrence_stem: None,
                predicate: "fought_at".into(),
                place_identity_qid: Some("Q6258".into()),
            },
        )
        .await
        .unwrap();
        crate::insert_person_quote_evidence(
            &pool,
            lodi,
            lodi_quote,
            None,
            0.9,
            "https://example.test/lodi",
        )
        .await
        .unwrap();
        let lodi_logical = logical_event_id_of(&pool, lodi).await.unwrap().unwrap();
        let lodi_successor = crate::supersede_person_event(
            &pool,
            lodi,
            &crate::PersonSupersession {
                title: "Battle of Lodi".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({
                    "kind": "exact", "precision": "day", "start": "1796-05-10", "surface": "10 May 1796"
                }),
                start_time: Some(utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
                place_label: Some("Lodi".into()),
                lat: Some(45.314),
                lon: Some(9.503),
                map_eligible: true,
                place_identity_qid: Some("Q6258".into()),
                evidence_quote: lodi_quote.into(),
                expected_fingerprint: Some("logical-lodi".into()),
                date_precision: Some("day".into()),
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            logical_event_id_of(&pool, lodi_successor).await.unwrap(),
            Some(lodi_logical)
        );
        assert_eq!(
            crate::resolve_active_person_event(&pool, lodi)
                .await
                .unwrap(),
            lodi_successor
        );

        let mondovi_quote = "The Battle of Mondovì was fought on 21 April 1796 near Mondovì.";
        let mondovi = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: napoleon,
                event_type: "battle".into(),
                epistemic_status: "attested".into(),
                title: "Battle of Mondovì".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1796, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "exact", "precision": "year", "start": "1796"}),
                place_label: Some("Mondovì".into()),
                lat: Some(59.6017),
                lon: Some(29.6686),
                confidence: 0.9,
                map_eligible: true,
                fingerprint: "logical-mondovi".into(),
                occurrence_key: "logical-mondovi-year".into(),
                occurrence_stem: None,
                predicate: "fought_at".into(),
                place_identity_qid: Some("Q50775".into()),
            },
        )
        .await
        .unwrap();
        crate::insert_person_quote_evidence(
            &pool,
            mondovi,
            mondovi_quote,
            None,
            0.9,
            "https://example.test/mondovi",
        )
        .await
        .unwrap();
        let mondovi_logical = logical_event_id_of(&pool, mondovi).await.unwrap().unwrap();
        assert_ne!(lodi_logical, mondovi_logical);
        let mondovi_successor = crate::supersede_person_event(
            &pool,
            mondovi,
            &crate::PersonSupersession {
                title: "Battle of Mondovì".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({
                    "kind": "exact", "precision": "day", "start": "1796-04-21", "surface": "21 April 1796"
                }),
                start_time: Some(utc.with_ymd_and_hms(1796, 4, 21, 0, 0, 0).unwrap()),
                place_label: Some("Mondovì".into()),
                lat: Some(44.3833),
                lon: Some(7.8167),
                map_eligible: true,
                place_identity_qid: Some("Q50775".into()),
                evidence_quote: mondovi_quote.into(),
                expected_fingerprint: Some("logical-mondovi".into()),
                date_precision: Some("day".into()),
                location_precision: Some("approximate".into()),
                uncertainty_radius_m: Some(5000.0),
                coord_precision_level: Some("city".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            logical_event_id_of(&pool, mondovi_successor).await.unwrap(),
            Some(mondovi_logical)
        );
        let restored = crate::revert_person_supersession(&pool, mondovi_successor)
            .await
            .unwrap();
        assert_eq!(restored, mondovi);
        let parked: (Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as(
            "SELECT supersedes, superseded_by, is_active FROM canonical_events WHERE id = $1",
        )
        .bind(mondovi_successor)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(parked, (Some(mondovi), Some(mondovi), false));
        assert_eq!(
            crate::resolve_active_person_event(&pool, mondovi_successor)
                .await
                .unwrap(),
            mondovi
        );
        assert_eq!(
            logical_event_id_of(&pool, mondovi).await.unwrap(),
            Some(mondovi_logical)
        );
        assert_eq!(
            logical_event_id_of(&pool, mondovi_successor).await.unwrap(),
            Some(mondovi_logical)
        );
        let mondovi_again = crate::supersede_person_event(
            &pool,
            mondovi,
            &crate::PersonSupersession {
                title: "Battle of Mondovì".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({
                    "kind": "exact", "precision": "day", "start": "1796-04-21", "surface": "21 April 1796"
                }),
                start_time: Some(utc.with_ymd_and_hms(1796, 4, 21, 0, 0, 0).unwrap()),
                place_label: Some("Mondovì".into()),
                lat: Some(44.3833),
                lon: Some(7.8167),
                map_eligible: true,
                place_identity_qid: Some("Q50775".into()),
                evidence_quote: mondovi_quote.into(),
                expected_fingerprint: Some("logical-mondovi".into()),
                date_precision: Some("day".into()),
                location_precision: Some("approximate".into()),
                uncertainty_radius_m: Some(5000.0),
                coord_precision_level: Some("city".into()),
            },
        )
        .await
        .unwrap();
        assert_ne!(mondovi_again, mondovi_successor);
        assert_eq!(
            logical_event_id_of(&pool, mondovi_again).await.unwrap(),
            Some(mondovi_logical)
        );

        let paper = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: curie,
                event_type: "publication".into(),
                epistemic_status: "attested".into(),
                title: "Research on radioactivity".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1898, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "exact", "precision": "year", "start": "1898"}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.8,
                map_eligible: false,
                fingerprint: "logical-curie-paper".into(),
                occurrence_key: "logical-curie-paper".into(),
                occurrence_stem: None,
                predicate: "published".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        let paper_logical = logical_event_id_of(&pool, paper).await.unwrap().unwrap();
        let recognized = crate::find_recognized_person_event(&pool, curie, "logical-curie-paper")
            .await
            .unwrap();
        assert_eq!(recognized, Some(paper));
        let refined = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: curie,
                event_type: "publication".into(),
                epistemic_status: "attested".into(),
                title: "Research on radioactivity".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1898, 7, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "exact", "precision": "month", "start": "1898-07"}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.8,
                map_eligible: false,
                fingerprint: "logical-curie-paper-refined".into(),
                occurrence_key: "logical-curie-paper-refined".into(),
                occurrence_stem: None,
                predicate: "published".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        assert_ne!(
            logical_event_id_of(&pool, refined).await.unwrap(),
            Some(paper_logical)
        );
        assert_eq!(
            crate::find_recognized_person_event(&pool, curie, "logical-curie-paper-refined")
                .await
                .unwrap(),
            Some(refined)
        );
        assert_eq!(
            crate::find_recognized_person_event(&pool, curie, "logical-curie-paper-nearby")
                .await
                .unwrap(),
            None
        );

        let exile = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: hugo,
                event_type: "exile".into(),
                epistemic_status: "attested".into(),
                title: "Exile in Guernsey".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1855, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "exact", "precision": "year", "start": "1855"}),
                place_label: Some("Guernsey".into()),
                lat: None,
                lon: None,
                confidence: 0.8,
                map_eligible: false,
                fingerprint: "logical-hugo-exile".into(),
                occurrence_key: "logical-hugo-exile".into(),
                occurrence_stem: None,
                predicate: "exiled_to".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        assert!(logical_event_id_of(&pool, exile).await.unwrap().is_some());

        let supper_quote =
            "Leonardo revised the Last Supper study at the Milan workshop during his workshop years.";
        let supper = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: leonardo,
                event_type: "publication".into(),
                epistemic_status: "attested".into(),
                title: "The Last Supper study".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1495, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "approx", "year": 1495, "surface": "about 1495"}),
                place_label: Some("Milan".into()),
                lat: Some(45.4642),
                lon: Some(9.19),
                confidence: 0.7,
                map_eligible: true,
                fingerprint: "logical-leonardo-supper".into(),
                occurrence_key: "logical-leonardo-supper".into(),
                occurrence_stem: None,
                predicate: "created".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        crate::insert_person_quote_evidence(
            &pool,
            supper,
            supper_quote,
            None,
            0.7,
            "https://example.test/supper",
        )
        .await
        .unwrap();
        let machine = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: leonardo,
                event_type: "historical_fact".into(),
                epistemic_status: "uncertain".into(),
                title: "Notes on a flying machine".into(),
                summary: None,
                start_time: Some(utc.with_ymd_and_hms(1485, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({"kind": "approx", "year": 1485}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.4,
                map_eligible: false,
                fingerprint: "logical-leonardo-machine".into(),
                occurrence_key: "logical-leonardo-machine".into(),
                occurrence_stem: None,
                predicate: "studied".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        let supper_logical = logical_event_id_of(&pool, supper).await.unwrap().unwrap();
        let machine_logical = logical_event_id_of(&pool, machine).await.unwrap().unwrap();
        assert_ne!(supper_logical, machine_logical);
        let supper_successor = crate::supersede_person_event(
            &pool,
            supper,
            &crate::PersonSupersession {
                title: "The Last Supper, workshop study".into(),
                event_type: "publication".into(),
                time_json: serde_json::json!({"kind": "approx", "year": 1495, "surface": "about 1495"}),
                start_time: Some(utc.with_ymd_and_hms(1495, 1, 1, 0, 0, 0).unwrap()),
                place_label: Some("Milan workshop".into()),
                lat: Some(45.4642),
                lon: Some(9.19),
                map_eligible: true,
                place_identity_qid: None,
                evidence_quote: supper_quote.into(),
                expected_fingerprint: Some("logical-leonardo-supper".into()),
                date_precision: None,
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            logical_event_id_of(&pool, supper_successor).await.unwrap(),
            Some(supper_logical)
        );
        assert_eq!(
            logical_event_id_of(&pool, machine).await.unwrap(),
            Some(machine_logical)
        );

        let lyceum = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: aristotle,
                event_type: "education".into(),
                epistemic_status: "uncertain".into(),
                title: "Teaching at the Lyceum".into(),
                summary: None,
                start_time: None,
                time_json: serde_json::json!({"kind": "unknown", "surface": "date unknown"}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.3,
                map_eligible: false,
                fingerprint: "logical-aristotle-lyceum".into(),
                occurrence_key: "logical-aristotle-lyceum".into(),
                occurrence_stem: None,
                predicate: "taught_at".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        let works = crate::insert_person_event(
            &pool,
            &crate::PersonEventInsert {
                entity_id: aristotle,
                event_type: "publication".into(),
                epistemic_status: "uncertain".into(),
                title: "Writings on nature".into(),
                summary: None,
                start_time: None,
                time_json: serde_json::json!({"kind": "approx", "surface": "4th century BCE"}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.3,
                map_eligible: false,
                fingerprint: "logical-aristotle-works".into(),
                occurrence_key: "logical-aristotle-works".into(),
                occurrence_stem: None,
                predicate: "wrote".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        let lyceum_logical = logical_event_id_of(&pool, lyceum).await.unwrap().unwrap();
        let works_logical = logical_event_id_of(&pool, works).await.unwrap().unwrap();
        assert_ne!(lyceum_logical, works_logical);

        let bare_a = Uuid::from_u128(501);
        let bare_b = Uuid::from_u128(502);
        let bare_c = Uuid::from_u128(503);
        sqlx::query(
            r#"
            INSERT INTO canonical_events (
                id, entity_id, event_type, title, time_json, pipeline, is_active,
                supersedes, superseded_by, occurrence_key, fingerprint
            )
            VALUES
                ($1, $4, 'historical_fact', 'Same displayed title', '{}', 'person', false, NULL, $2, 'bare-shared-key', 'bare-fp-a'),
                ($2, $4, 'historical_fact', 'Same displayed title', '{}', 'person', true, $1, NULL, 'bare-other-key', 'bare-fp-b'),
                ($3, $4, 'historical_fact', 'Unrelated reading', '{}', 'person', false, NULL, NULL, 'bare-shared-key', 'bare-fp-c')
            "#,
        )
        .bind(bare_a)
        .bind(bare_b)
        .bind(bare_c)
        .bind(napoleon)
        .execute(&pool)
        .await
        .unwrap();

        let (first, second) = tokio::join!(
            backfill_logical_identities(&pool),
            backfill_logical_identities(&pool)
        );
        let first = first.unwrap();
        let second = second.unwrap();
        let ambiguous = Uuid::from_u128(601);
        let ambiguous_fan = Uuid::from_u128(602);
        let forward_archive = Uuid::from_u128(603);
        sqlx::query(
            r#"
            INSERT INTO canonical_events (
                id, entity_id, event_type, title, time_json, pipeline, is_active,
                supersedes, superseded_by, occurrence_key, fingerprint
            )
            VALUES
                ($1, $4, 'publication', 'Research on radioactivity', '{}', 'person', false, NULL, $5, 'ambiguous-key-a', 'ambiguous-fp-a'),
                ($2, $4, 'publication', 'Research on radioactivity', '{}', 'person', false, NULL, $5, 'ambiguous-key-b', 'ambiguous-fp-b'),
                ($3, $6, 'exile', 'Exile in Guernsey', '{}', 'person', false, NULL, $7, 'forward-archive-key', 'forward-archive-fp')
            "#,
        )
        .bind(ambiguous)
        .bind(ambiguous_fan)
        .bind(forward_archive)
        .bind(curie)
        .bind(paper)
        .bind(hugo)
        .bind(exile)
        .execute(&pool)
        .await
        .unwrap();

        let assigned = first.assigned_events + second.assigned_events;
        assert_eq!(assigned, 2);
        let chain_a = logical_event_id_of(&pool, bare_a).await.unwrap().unwrap();
        let chain_b = logical_event_id_of(&pool, bare_b).await.unwrap().unwrap();
        assert!(logical_event_id_of(&pool, bare_c).await.unwrap().is_none());
        assert_eq!(chain_a, chain_b);
        assert_ne!(chain_a, lodi_logical);
        assert!(logical_event_id_of(&pool, ambiguous)
            .await
            .unwrap()
            .is_none());
        assert!(logical_event_id_of(&pool, ambiguous_fan)
            .await
            .unwrap()
            .is_none());
        assert!(logical_event_id_of(&pool, forward_archive)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            logical_event_id_of(&pool, paper).await.unwrap(),
            Some(paper_logical)
        );
        assert_eq!(
            crate::resolve_active_person_event(&pool, ambiguous)
                .await
                .unwrap(),
            ambiguous
        );
        let unresolved = crate::resolve_person_event(&pool, ambiguous).await.unwrap();
        assert_eq!(unresolved.filiation, crate::FILIATION_UNVERIFIED);
        assert_eq!(unresolved.linked_event_id, Some(paper));
        assert_eq!(
            crate::resolve_person_event(&pool, lodi)
                .await
                .unwrap()
                .filiation,
            crate::FILIATION_VERIFIED
        );
        assert_eq!(
            crate::resolve_active_person_event(&pool, forward_archive)
                .await
                .unwrap(),
            forward_archive
        );

        let repeated = backfill_logical_identities(&pool).await.unwrap();
        assert_eq!(repeated.assigned_events, 0);
        assert!(logical_event_id_of(&pool, ambiguous)
            .await
            .unwrap()
            .is_none());
        assert!(logical_event_id_of(&pool, forward_archive)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            logical_event_id_of(&pool, paper).await.unwrap(),
            Some(paper_logical)
        );
        assert_eq!(
            logical_event_id_of(&pool, bare_a).await.unwrap(),
            Some(chain_a)
        );
        assert_eq!(
            logical_event_id_of(&pool, lodi).await.unwrap(),
            Some(lodi_logical)
        );
        assert_eq!(
            logical_event_id_of(&pool, mondovi_successor).await.unwrap(),
            Some(mondovi_logical)
        );
        assert_eq!(
            crate::resolve_active_person_event(&pool, mondovi_successor)
                .await
                .unwrap(),
            mondovi_again
        );
        let report = diagnose_logical_identities(&pool).await.unwrap();
        assert!(!report.blocks_migration(), "{:?}", report.issues);
        assert!(report.parked_restorations >= 1);
        assert!(report.inactive_unlinked >= 1);
        let proposals = crate::propose_logical_reconciliations(&pool).await.unwrap();
        assert!(!proposals.iter().any(|proposal| {
            let pair = [proposal.left_event_id, proposal.right_event_id];
            pair.contains(&paper) && pair.contains(&refined)
        }));
        assert!(
            logical_event_id_of(&pool, paper).await.unwrap()
                != logical_event_id_of(&pool, refined).await.unwrap()
        );
    }
}
