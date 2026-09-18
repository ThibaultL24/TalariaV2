// crates/talaria-api/src/intuition.rs
//! Plan / export / publish situated debates to Intuition (opinions only).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use talaria_core::AppConfig;
use talaria_intuition::{
    fact_fingerprint, fact_from_place_conflict, fact_from_soft_claim, ConflictGroup, DebateFact,
    SoftClaimInput, SCHEMA_VERSION_V2,
};
use talaria_store::{
    connect, find_entity_by_wikipedia_title, find_quality_event_for_stem, get_quality_event_pointer,
    list_conflict_quality_claims, list_exportable_soft_claims, run_migrations, search_local_entities,
    upsert_intuition_publication, IntuitionPublicationInsert,
};
use uuid::Uuid;

pub const LIVE_PUBLISH_BLOCKED: &str =
    "live_disabled: intuition-publish --live is blocked until IPFS atom IDs and tx simulation are fixed";

pub fn live_publish_guard(live: bool) -> anyhow::Result<()> {
    if live {
        anyhow::bail!(LIVE_PUBLISH_BLOCKED);
    }
    Ok(())
}

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

async fn collect_facts(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    subject_label: &str,
) -> anyhow::Result<Vec<DebateFact>> {
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
            out.push(fact_from_place_conflict(&group, place));
        }
    }

    for row in list_exportable_soft_claims(pool, subject_id).await? {
        let pointer = match row.canonical_event_id {
            Some(eid) => get_quality_event_pointer(pool, eid).await?,
            None => None,
        };
        out.push(fact_from_soft_claim(&SoftClaimInput {
            subject_label: subject_label.into(),
            claim_id: row.id.to_string(),
            claim_kind: row.claim_kind,
            text: row.text,
            event_id: pointer.as_ref().map(|p| p.id.to_string()),
            event_title: pointer.as_ref().map(|p| p.title.clone()),
            place_label: row.place_label.or(pointer.as_ref().and_then(|p| p.place_label.clone())),
            event_type: pointer.as_ref().map(|p| p.event_type.clone()),
            time_surface: pointer.as_ref().map(|p| time_key(&p.time_json)),
        }));
    }
    Ok(out)
}

async fn model_fact(fact: &DebateFact) -> anyhow::Result<serde_json::Value> {
    let out = spawn_sidecar("model", fact).await?;
    if out.get("status").and_then(|s| s.as_str()) != Some("ok") {
        anyhow::bail!("sidecar model status not ok: {out}");
    }
    Ok(out.get("graph").cloned().unwrap_or(out))
}

async fn persist_pending(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    facts: &[DebateFact],
    graphs: &[serde_json::Value],
) -> anyhow::Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for (fact, graph) in facts.iter().zip(graphs.iter()) {
        let fp = fact_fingerprint(fact);
        let id = upsert_intuition_publication(
            pool,
            &IntuitionPublicationInsert {
                subject_entity_id: subject_id,
                debate_id: fact.debate_id.clone(),
                bundle_fingerprint: fp,
                kind: fact.kind.clone(),
                status: "pending".into(),
                payload_json: serde_json::json!({
                    "version": SCHEMA_VERSION_V2,
                    "fact": fact,
                    "graph": graph,
                }),
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
    let facts = collect_facts(&pool, id, &label).await?;
    let mut graphs = Vec::new();
    for fact in &facts {
        graphs.push(model_fact(fact).await?);
    }
    persist_pending(&pool, id, &facts, &graphs).await?;
    let report = serde_json::json!({
        "version": SCHEMA_VERSION_V2,
        "subject": label,
        "entity_id": id,
        "debate_count": facts.len(),
        "debates": facts.iter().zip(graphs.iter()).map(|(f, g)| serde_json::json!({
            "kind": f.kind,
            "debate_id": f.debate_id,
            "question": f.question.text,
            "proposition": f.proposition.text,
            "category": g.get("category"),
            "voteTripleId": g.get("voteTripleId"),
            "eventAtom": g.get("eventAtom"),
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub async fn run_intuition_export(config: &AppConfig, subject: &str) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let facts = collect_facts(&pool, id, &label).await?;
    let mut graphs = Vec::new();
    for fact in &facts {
        graphs.push(model_fact(fact).await?);
    }
    persist_pending(&pool, id, &facts, &graphs).await?;
    let report = serde_json::json!({
        "version": SCHEMA_VERSION_V2,
        "subject": label,
        "entity_id": id,
        "debates": facts.iter().zip(graphs.iter()).map(|(f, g)| serde_json::json!({
            "fact": f,
            "graph": g,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub async fn run_intuition_publish(
    config: &AppConfig,
    subject: &str,
    live: bool,
) -> anyhow::Result<()> {
    live_publish_guard(live)?;
    run_intuition_export(config, subject).await
}

async fn spawn_sidecar(mode: &str, fact: &DebateFact) -> anyhow::Result<serde_json::Value> {
    let script = PathBuf::from(
        std::env::var("INTUITION_SIDECAR").unwrap_or_else(|_| "sidecar/intuition/cli.ts".into()),
    );
    let dir = script
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let file = script
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("cli.ts");
    let payload = serde_json::to_string(fact)?;
    let tsx = dir.join("node_modules/.bin/tsx");
    let mut cmd = if tsx.exists() {
        let mut c = tokio::process::Command::new(tsx);
        c.args([file, mode, &payload]);
        c
    } else {
        let mut c = tokio::process::Command::new("npx");
        c.args(["tsx", file, mode, &payload]);
        c
    };
    let child = cmd
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
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or(serde_json::json!({}));
    if parsed.get("status").and_then(|s| s.as_str()) == Some("ok")
        || parsed.get("status").and_then(|s| s.as_str()) == Some("pin_failed")
        || parsed.get("status").and_then(|s| s.as_str()) == Some("failed")
    {
        return Ok(parsed);
    }
    if !output.status.success() {
        anyhow::bail!(
            "sidecar failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        );
    }
    anyhow::bail!("sidecar non-JSON stdout: {stdout}; stderr={stderr}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_publish_is_blocked() {
        let err = live_publish_guard(true).unwrap_err().to_string();
        assert!(err.contains("live_disabled"));
        assert!(live_publish_guard(false).is_ok());
    }
}
