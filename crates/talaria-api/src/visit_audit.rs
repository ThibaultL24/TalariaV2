// crates/talaria-api/src/visit_audit.rs
use serde::Serialize;
use sqlx::PgPool;
use talaria_store::{
    deactivate_canonical_event, list_visit_heritage_for_audit, visit_heritage_source_counts,
    VisitHeritageAuditRow,
};
use uuid::Uuid;

const PROXIMITY_DEDUP_M: f64 = 80.0;

#[derive(Debug, Serialize)]
pub struct VisitAuditReport {
    pub entity_id: Uuid,
    pub label: String,
    pub qid: Option<String>,
    pub timeline_eligible: i64,
    pub map_eligible: i64,
    pub visit_heritage_total: i64,
    pub visit_from_fixture: i64,
    pub visit_from_wikidata: i64,
    pub visit_from_ingest: i64,
    pub demo_fixture_rows_for_qid: usize,
    pub ingest_exceeds_demo_visit: bool,
    pub proximity_clusters: Vec<ProximityCluster>,
    pub dedupe_deactivated: usize,
}

#[derive(Debug, Serialize)]
pub struct ProximityCluster {
    pub event_type: String,
    pub lat: f64,
    pub lon: f64,
    pub kept_id: Uuid,
    pub kept_title: String,
    pub removed_ids: Vec<Uuid>,
    pub removed_titles: Vec<String>,
}

fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0;
    let to_rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let d_lat = to_rad(lat2 - lat1);
    let d_lon = to_rad(lon2 - lon1);
    let a = (d_lat / 2.0).sin().powi(2)
        + to_rad(lat1).cos() * to_rad(lat2).cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().asin()
}

fn count_demo_fixture_rows(qid: &str) -> usize {
    let path = crate::cli_helpers::resolve_repo_path(
        std::path::PathBuf::from("fixtures/visit/heritage_demo.json"),
    );
    if !path.is_file() {
        return 0;
    }
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    let rows: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap_or_default();
    rows.iter()
        .filter(|r| r.get("qid").and_then(|v| v.as_str()) == Some(qid))
        .count()
}

pub async fn run_visit_audit(
    pool: &PgPool,
    entity_id: Uuid,
    label: String,
    qid: Option<String>,
    apply_dedupe: bool,
) -> anyhow::Result<VisitAuditReport> {
    let (visit_total, fixture_n, wikidata_n, ingest_n, timeline_n, map_n) =
        visit_heritage_source_counts(pool, entity_id).await?;
    let demo_rows = qid
        .as_deref()
        .map(count_demo_fixture_rows)
        .unwrap_or(0);
    let rows = list_visit_heritage_for_audit(pool, entity_id).await?;
    let (clusters, to_deactivate) = plan_proximity_dedupe(&rows);

    let mut dedupe_deactivated = 0usize;
    if apply_dedupe {
        for id in to_deactivate {
            deactivate_canonical_event(pool, id).await?;
            dedupe_deactivated += 1;
        }
    }

    Ok(VisitAuditReport {
        entity_id,
        label,
        qid,
        timeline_eligible: timeline_n,
        map_eligible: map_n,
        visit_heritage_total: visit_total,
        visit_from_fixture: fixture_n,
        visit_from_wikidata: wikidata_n,
        visit_from_ingest: ingest_n,
        demo_fixture_rows_for_qid: demo_rows,
        ingest_exceeds_demo_visit: ingest_n > demo_rows as i64,
        proximity_clusters: clusters,
        dedupe_deactivated,
    })
}

fn plan_proximity_dedupe(
    rows: &[VisitHeritageAuditRow],
) -> (Vec<ProximityCluster>, Vec<Uuid>) {
    let mut clusters = Vec::new();
    let mut to_deactivate = Vec::new();
    let mut used = vec![false; rows.len()];

    for i in 0..rows.len() {
        if used[i] {
            continue;
        }
        let a = &rows[i];
        let (alat, alon) = match (a.lat, a.lon) {
            (Some(lat), Some(lon)) if lat.is_finite() && lon.is_finite() => (lat, lon),
            _ => continue,
        };
        let mut group = vec![i];
        for j in (i + 1)..rows.len() {
            if used[j] {
                continue;
            }
            let b = &rows[j];
            if a.event_type != b.event_type {
                continue;
            }
            let (blat, blon) = match (b.lat, b.lon) {
                (Some(lat), Some(lon)) if lat.is_finite() && lon.is_finite() => (lat, lon),
                _ => continue,
            };
            if haversine_m(alat, alon, blat, blon) <= PROXIMITY_DEDUP_M {
                group.push(j);
            }
        }
        if group.len() < 2 {
            continue;
        }
        let mut ranked = group.clone();
        ranked.sort_by(|&x, &y| {
            rows[y]
                .evidence_count
                .cmp(&rows[x].evidence_count)
                .then_with(|| rows[x].title.cmp(&rows[y].title))
        });
        let keep = ranked[0];
        let removed: Vec<usize> = ranked[1..].to_vec();
        for idx in &group {
            used[*idx] = true;
        }
        for idx in &removed {
            to_deactivate.push(rows[*idx].id);
        }
        clusters.push(ProximityCluster {
            event_type: rows[keep].event_type.clone(),
            lat: alat,
            lon: alon,
            kept_id: rows[keep].id,
            kept_title: rows[keep].title.clone(),
            removed_ids: removed.iter().map(|i| rows[*i].id).collect(),
            removed_titles: removed.iter().map(|i| rows[*i].title.clone()).collect(),
        });
    }
    (clusters, to_deactivate)
}
