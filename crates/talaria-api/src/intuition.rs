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
    connect, find_entity_by_wikipedia_title, find_person_event_for_stem, get_person_event_pointer,
    list_conflict_quality_claims, list_exportable_soft_claims, mark_intuition_failed,
    mark_intuition_pin_failed, mark_intuition_published, run_migrations, search_local_entities,
    upsert_intuition_publication, upsert_intuition_term_binding, IntuitionPublicationInsert,
    IntuitionTermBindingInsert,
};
use uuid::Uuid;

pub const LIVE_PUBLISH_BLOCKED: &str =
    "live_disabled: set INTUITION_ALLOW_LIVE=1 and INTUITION_PRIVATE_KEY to enable testnet publish";

/// Demo / operator opt-in for Intuition testnet settle (`intuition-publish --live`).
pub fn live_publish_allowed() -> bool {
    matches!(
        std::env::var("INTUITION_ALLOW_LIVE")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on")
    )
}

pub fn live_publish_guard(live: bool) -> anyhow::Result<()> {
    if live && !live_publish_allowed() {
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

struct CollectedFact {
    fact: DebateFact,
    soft_claim_id: Option<Uuid>,
}

async fn collect_facts(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    subject_label: &str,
) -> anyhow::Result<Vec<CollectedFact>> {
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
            get_person_event_pointer(pool, eid).await?
        } else {
            find_person_event_for_stem(pool, subject_id, &stem).await?
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
            out.push(CollectedFact {
                fact: fact_from_place_conflict(&group, place),
                soft_claim_id: None,
            });
        }
    }

    // Cap export volume so Agora densify does not flood the Intuition queue.
    const MAX_SOFT_CLAIMS: usize = 80;
    for row in list_exportable_soft_claims(pool, subject_id)
        .await?
        .into_iter()
        .take(MAX_SOFT_CLAIMS)
    {
        let pointer = match row.canonical_event_id {
            Some(eid) => get_person_event_pointer(pool, eid).await?,
            None => None,
        };
        out.push(CollectedFact {
            fact: fact_from_soft_claim(&SoftClaimInput {
                subject_label: subject_label.into(),
                claim_id: row.id.to_string(),
                claim_kind: row.claim_kind,
                text: row.text,
                event_id: pointer.as_ref().map(|p| p.id.to_string()),
                event_title: pointer.as_ref().map(|p| p.title.clone()),
                place_label: row
                    .place_label
                    .or(pointer.as_ref().and_then(|p| p.place_label.clone())),
                event_type: pointer.as_ref().map(|p| p.event_type.clone()),
                time_surface: pointer.as_ref().map(|p| time_key(&p.time_json)),
            }),
            soft_claim_id: Some(row.id),
        });
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
    collected: &[CollectedFact],
    graphs: &[serde_json::Value],
) -> anyhow::Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for (item, graph) in collected.iter().zip(graphs.iter()) {
        let fact = &item.fact;
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
                soft_claim_id: item.soft_claim_id,
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
    let collected = collect_facts(&pool, id, &label).await?;
    let mut graphs = Vec::new();
    for item in &collected {
        graphs.push(model_fact(&item.fact).await?);
    }
    persist_pending(&pool, id, &collected, &graphs).await?;
    let report = serde_json::json!({
        "version": SCHEMA_VERSION_V2,
        "subject": label,
        "entity_id": id,
        "debate_count": collected.len(),
        "debates": collected.iter().zip(graphs.iter()).map(|(item, g)| serde_json::json!({
            "kind": item.fact.kind,
            "debate_id": item.fact.debate_id,
            "question": item.fact.question.text,
            "proposition": item.fact.proposition.text,
            "category": g.get("category"),
            "voteTripleRole": g.get("voteTripleRole"),
            "atoms": g.get("atoms"),
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub async fn run_intuition_export(config: &AppConfig, subject: &str) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let collected = collect_facts(&pool, id, &label).await?;
    let mut graphs = Vec::new();
    for item in &collected {
        graphs.push(model_fact(&item.fact).await?);
    }
    persist_pending(&pool, id, &collected, &graphs).await?;
    let report = serde_json::json!({
        "version": SCHEMA_VERSION_V2,
        "subject": label,
        "entity_id": id,
        "debates": collected.iter().zip(graphs.iter()).map(|(item, g)| serde_json::json!({
            "fact": item.fact,
            "graph": g,
            "soft_claim_id": item.soft_claim_id,
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
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (id, label) = resolve_subject(&pool, &config.wiki_lang, subject).await?;
    let collected = collect_facts(&pool, id, &label).await?;
    let mut graphs = Vec::new();
    for item in &collected {
        graphs.push(model_fact(&item.fact).await?);
    }
    let pub_ids = persist_pending(&pool, id, &collected, &graphs).await?;

    if !live {
        let report = serde_json::json!({
            "version": SCHEMA_VERSION_V2,
            "subject": label,
            "entity_id": id,
            "mode": "dry_run",
            "debate_count": collected.len(),
            "publication_ids": pub_ids,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let mut results = Vec::new();
    for (item, pub_id) in collected.iter().zip(pub_ids.iter()) {
        let out = spawn_sidecar("publish", &item.fact).await?;
        let status = out.get("status").and_then(|s| s.as_str()).unwrap_or("failed");
        match status {
            "published" => {
                let chain_id = out
                    .get("chainId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let question_term = out
                    .get("atoms")
                    .and_then(|a| a.as_array())
                    .and_then(|arr| {
                        arr.iter().find(|x| {
                            x.get("role").and_then(|r| r.as_str()) == Some("question")
                        })
                    })
                    .and_then(|x| x.get("termId").and_then(|t| t.as_str()));
                let vote_triple = out
                    .get("triples")
                    .and_then(|a| a.as_array())
                    .and_then(|arr| {
                        arr.iter().find(|x| {
                            x.get("role").and_then(|r| r.as_str())
                                == Some("question_has_proposition")
                        })
                    });
                let triple_term = vote_triple.and_then(|x| x.get("termId").and_then(|t| t.as_str()));
                let tx_hash = vote_triple.and_then(|x| x.get("txHash").and_then(|t| t.as_str()));
                mark_intuition_published(
                    &pool,
                    *pub_id,
                    chain_id,
                    question_term,
                    triple_term,
                    tx_hash,
                )
                .await?;
                if let Some(atoms) = out.get("atoms").and_then(|a| a.as_array()) {
                    for atom in atoms {
                        let Some(role) = atom.get("role").and_then(|r| r.as_str()) else {
                            continue;
                        };
                        let Some(term_id) = atom.get("termId").and_then(|t| t.as_str()) else {
                            continue;
                        };
                        upsert_intuition_term_binding(
                            &pool,
                            &IntuitionTermBindingInsert {
                                publication_id: *pub_id,
                                role: role.into(),
                                local_kind: "atom".into(),
                                local_id: item.soft_claim_id.map(|u| u.to_string()),
                                chain_id,
                                term_id: term_id.into(),
                                ipfs_uri: atom
                                    .get("ipfsUri")
                                    .and_then(|u| u.as_str())
                                    .map(str::to_string),
                                data_hash: None,
                                classification: None,
                                created_on_chain: atom
                                    .get("created")
                                    .and_then(|c| c.as_bool())
                                    .unwrap_or(false),
                                verified: atom
                                    .get("verified")
                                    .and_then(|c| c.as_bool())
                                    .unwrap_or(false),
                            },
                        )
                        .await?;
                    }
                }
                if let Some(triples) = out.get("triples").and_then(|a| a.as_array()) {
                    for triple in triples {
                        let Some(role) = triple.get("role").and_then(|r| r.as_str()) else {
                            continue;
                        };
                        let Some(term_id) = triple.get("termId").and_then(|t| t.as_str()) else {
                            continue;
                        };
                        upsert_intuition_term_binding(
                            &pool,
                            &IntuitionTermBindingInsert {
                                publication_id: *pub_id,
                                role: role.into(),
                                local_kind: "triple".into(),
                                local_id: None,
                                chain_id,
                                term_id: term_id.into(),
                                ipfs_uri: None,
                                data_hash: None,
                                classification: None,
                                created_on_chain: triple
                                    .get("created")
                                    .and_then(|c| c.as_bool())
                                    .unwrap_or(false),
                                verified: triple
                                    .get("verified")
                                    .and_then(|c| c.as_bool())
                                    .unwrap_or(false),
                            },
                        )
                        .await?;
                    }
                }
                results.push(serde_json::json!({
                    "publication_id": pub_id,
                    "status": "published",
                    "debate_id": item.fact.debate_id,
                }));
            }
            "pin_failed" => {
                let msg = out
                    .get("message")
                    .or_else(|| out.get("error"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("pin_failed");
                mark_intuition_pin_failed(&pool, *pub_id, msg).await?;
                results.push(serde_json::json!({
                    "publication_id": pub_id,
                    "status": "pin_failed",
                    "message": msg,
                }));
            }
            _ => {
                let msg = out
                    .get("message")
                    .or_else(|| out.get("error"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("failed");
                mark_intuition_failed(&pool, *pub_id, msg).await?;
                results.push(serde_json::json!({
                    "publication_id": pub_id,
                    "status": "failed",
                    "message": msg,
                }));
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "version": SCHEMA_VERSION_V2,
            "subject": label,
            "entity_id": id,
            "mode": "live",
            "results": results,
        }))?
    );
    Ok(())
}

async fn spawn_sidecar(mode: &str, fact: &DebateFact) -> anyhow::Result<serde_json::Value> {
    let payload = serde_json::to_string(fact)?;
    spawn_sidecar_payload(mode, &payload).await
}

pub async fn run_stance_preview(
    vote_triple_id: &str,
    stance: crate::agora_stance::StanceKind,
) -> anyhow::Result<serde_json::Value> {
    let stance = match stance {
        crate::agora_stance::StanceKind::Believe => "believe",
        crate::agora_stance::StanceKind::Dispute => "dispute",
    };
    let payload = serde_json::json!({
        "voteTripleId": vote_triple_id,
        "stance": stance,
    })
    .to_string();
    spawn_sidecar_payload("stance-preview", &payload).await
}

async fn spawn_sidecar_payload(mode: &str, payload: &str) -> anyhow::Result<serde_json::Value> {
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
    let tsx = dir.join("node_modules/.bin/tsx");
    let mut cmd = if tsx.exists() {
        let mut c = tokio::process::Command::new(tsx);
        c.args([file, mode, payload]);
        c
    } else {
        let mut c = tokio::process::Command::new("npx");
        c.args(["tsx", file, mode, payload]);
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
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or(serde_json::json!({}));
    let status = parsed.get("status").and_then(|s| s.as_str()).unwrap_or("");
    if matches!(
        status,
        "ok" | "published" | "previewed" | "deposited" | "pin_failed" | "failed"
    ) {
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
    fn live_publish_is_blocked_by_default() {
        // Ensure flag is off for this process (tests may inherit env).
        std::env::remove_var("INTUITION_ALLOW_LIVE");
        let err = live_publish_guard(true).unwrap_err().to_string();
        assert!(err.contains("live_disabled") || err.contains("INTUITION_ALLOW_LIVE"));
        assert!(live_publish_guard(false).is_ok());
    }

    #[test]
    fn live_publish_allowed_when_flag_set() {
        std::env::set_var("INTUITION_ALLOW_LIVE", "1");
        assert!(live_publish_allowed());
        assert!(live_publish_guard(true).is_ok());
        std::env::remove_var("INTUITION_ALLOW_LIVE");
    }
}
