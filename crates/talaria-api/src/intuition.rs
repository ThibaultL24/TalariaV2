// crates/talaria-api/src/intuition.rs
//! Plan / export / publish situated debates to Intuition (opinions only).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use talaria_core::AppConfig;
use talaria_intuition::{
    debate_from_place_conflict, debate_from_soft_claim, ConflictGroup, PlannedDebate, SoftClaimInput,
};
use talaria_store::{
    connect, find_entity_by_wikipedia_title, find_quality_event_for_stem,
    get_intuition_publication_by_fingerprint, get_quality_event_pointer, list_conflict_quality_claims,
    list_exportable_soft_claims, mark_intuition_failed, mark_intuition_published, run_migrations,
    search_local_entities, upsert_intuition_publication, IntuitionPublicationInsert,
};
use uuid::Uuid;

fn time_key(v: &serde_json::Value) -> String {
    if let Some(s) = v.get("surface").and_then(|x| x.as_str()).filter(|s| !s.is_empty()) {
        return s.to_string();
    }
    match (
        v.get("year").and_then(|x| x.as_i64()),
        v.get("month").and_then(|x| x.as_u64()),
        v.get("day").and_then(|x| x.as_u64()),
    ) {
        (Some(y), Some(m), Some(d)) => format!("{y:04}-{m:02}-{d:02}"),
        (Some(y), Some(m), None) => format!("{y:04}-{m:02}"),
        (Some(y), None, None) => y.to_string(),
        _ => "unknown".into(),
    }
}

async fn resolve_subject(
    pool: &sqlx::PgPool,
    wiki_lang: &str,
    subject: &str,
) -> anyhow::Result<(Uuid, String)> {
    if let Some(e) = find_entity_by_wikipedia_title(pool, wiki_lang, subject).await? {
        let label = e
            .canonical_name
            .clone()
            .unwrap_or(e.wikipedia_title.clone());
        return Ok((e.id, label));
    }
    let hits = search_local_entities(pool, subject, 1).await?;
    let Some(e) = hits.into_iter().next() else {
        anyhow::bail!("no entity matching {subject:?}");
    };
    let label = e
        .canonical_name
        .clone()
        .unwrap_or(e.wikipedia_title.clone());
    Ok((e.id, label))
}

async fn collect_debates(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    subject_label: &str,
) -> anyhow::Result<Vec<PlannedDebate>> {
    let mut out = Vec::new();
    let conflicts = list_conflict_quality_claims(pool, subject_id).await?;
    let mut by_stem: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for row in conflicts {
        let stem = match row.occurrence_stem.clone() {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        by_stem.entry(stem).or_default().push(row);
    }
    for (stem, rows) in by_stem {
        let mut places: Vec<String> = rows
            .iter()
            .filter_map(|r| r.place_label.clone())
            .filter(|p| !p.trim().is_empty())
            .collect();
        places.sort();
        places.dedup();
        if places.len() < 2 {
            continue;
        }
        let sample = &rows[0];
        let pointer = if let Some(eid) = sample.canonical_event_id {
            get_quality_event_pointer(pool, eid).await?
        } else {
            find_quality_event_for_stem(pool, subject_id, &stem).await?
        };
        let group = ConflictGroup {
            subject_label: subject_label.into(),
            occurrence_stem: stem,
            event_type: sample.event_type.clone(),
            time_key: time_key(&sample.time_json),
            places: places.clone(),
            event_id: pointer.as_ref().map(|p| p.id.to_string()),
            event_title: pointer.as_ref().map(|p| p.title.clone()),
        };
        for place in &places {
            out.push(debate_from_place_conflict(&group, place)?);
        }
    }

    for row in list_exportable_soft_claims(pool, subject_id).await? {
        let pointer = match row.canonical_event_id {
            Some(eid) => get_quality_event_pointer(pool, eid).await?,
            None => None,
        };
        out.push(debate_from_soft_claim(&SoftClaimInput {
            subject_label: subject_label.into(),
            claim_id: row.id.to_string(),
            claim_kind: row.claim_kind,
            text: row.text,
            event_id: pointer.as_ref().map(|p| p.id.to_string()),
            event_title: pointer.as_ref().map(|p| p.title.clone()),
            place_label: row.place_label.or(pointer.and_then(|p| p.place_label)),
        })?);
    }
    Ok(out)
}

async fn persist(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    debates: &[PlannedDebate],
    status: &str,
) -> anyhow::Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for d in debates {
        let fp = d.bundle.vote_target.triple_fingerprint.clone();
        let id = upsert_intuition_publication(
            pool,
            &IntuitionPublicationInsert {
                subject_entity_id: subject_id,
                debate_id: d.debate_id.clone(),
                bundle_fingerprint: fp,
                kind: d.kind.clone(),
                status: status.into(),
                payload_json: serde_json::to_value(d)?,
            },
        )
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

pub async fn run_intuition_plan(config: &AppConfig, subject: &str) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let debates = collect_debates(&pool, id, &label).await?;
    persist(&pool, id, &debates, "planned").await?;
    let report = serde_json::json!({
        "subject": label,
        "entity_id": id,
        "debate_count": debates.len(),
        "debates": debates.iter().map(|d| serde_json::json!({
            "kind": d.kind,
            "debate_id": d.debate_id,
            "question": d.question_label,
            "proposition": d.proposition_label,
            "vote_target": d.bundle.vote_target,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub async fn run_intuition_export(config: &AppConfig, subject: &str) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let debates = collect_debates(&pool, id, &label).await?;
    persist(&pool, id, &debates, "exported").await?;
    let report = serde_json::json!({
        "version": talaria_intuition::SCHEMA_VERSION,
        "subject": label,
        "entity_id": id,
        "debates": debates,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub async fn run_intuition_publish(
    config: &AppConfig,
    subject: &str,
    live: bool,
) -> anyhow::Result<()> {
    if !live {
        return run_intuition_export(config, subject).await;
    }
    let key = std::env::var("INTUITION_PRIVATE_KEY").unwrap_or_default();
    if !key.starts_with("0x") || key.len() != 66 {
        anyhow::bail!("INTUITION_PRIVATE_KEY must be a 0x-prefixed 32-byte hex key");
    }
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let debates = collect_debates(&pool, id, &label).await?;
    let pub_ids = persist(&pool, id, &debates, "exported").await?;
    let mut results = Vec::new();
    for (debate, pub_id) in debates.iter().zip(pub_ids.iter()) {
        let fp = &debate.bundle.vote_target.triple_fingerprint;
        if let Some(existing) = get_intuition_publication_by_fingerprint(&pool, fp).await? {
            if existing.status == "published" {
                results.push(serde_json::json!({
                    "debate_id": debate.debate_id,
                    "status": "already_published",
                    "triple_term_id": existing.triple_term_id,
                }));
                continue;
            }
        }
        let main = debate
            .bundle
            .triples
            .iter()
            .find(|t| t.role == "question_has_proposition")
            .ok_or_else(|| anyhow::anyhow!("missing vote-target triple"))?;
        let payload = serde_json::json!({
            "subject": main.subject,
            "predicate": main.predicate,
            "object": main.object,
            "positionKind": "believe",
        });
        match spawn_sidecar(&payload).await {
            Ok(out) => {
                let chain_id = out
                    .pointer("/network/observedChainId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(13579) as i32;
                let q_term = out
                    .pointer("/terms/questionAtom/termId")
                    .and_then(|v| v.as_str());
                let t_term = out
                    .pointer("/terms/mainTriple/termId")
                    .and_then(|v| v.as_str());
                let tx = out
                    .pointer("/tx/mainTriple")
                    .and_then(|v| v.as_str());
                mark_intuition_published(&pool, *pub_id, chain_id, q_term, t_term, tx).await?;
                results.push(serde_json::json!({
                    "debate_id": debate.debate_id,
                    "status": "published",
                    "sidecar": out,
                }));
            }
            Err(err) => {
                mark_intuition_failed(&pool, *pub_id, &err.to_string()).await?;
                results.push(serde_json::json!({
                    "debate_id": debate.debate_id,
                    "status": "failed",
                    "error": err.to_string(),
                }));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "subject": label,
            "live": true,
            "results": results,
        }))?
    );
    Ok(())
}

async fn spawn_sidecar(payload: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let script = PathBuf::from(
        std::env::var("INTUITION_SIDECAR")
            .unwrap_or_else(|_| "sidecar/intuition/writeOnChain.ts".into()),
    );
    let dir = script
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let file = script
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("writeOnChain.ts");
    let child = tokio::process::Command::new("npx")
        .args(["tsx", file, &payload.to_string()])
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to spawn intuition sidecar ({e}). Run: cd sidecar/intuition && npm install"
            )
        })?;
    let output = child.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        anyhow::bail!(
            "sidecar failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        );
    }
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|e| {
        anyhow::anyhow!("sidecar non-JSON stdout: {e}; stderr={stderr}")
    })?;
    if parsed.get("status").and_then(|s| s.as_str()) != Some("ok") {
        anyhow::bail!("sidecar status not ok: {parsed}");
    }
    Ok(parsed)
}
