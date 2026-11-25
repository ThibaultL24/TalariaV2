// crates/talaria-api/src/historical_review.rs
//! Read-only historical review of active person events.
//! The review selects rows and evidence. It does not insert or update them.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use talaria_ai::{
    exclusive_status, review_historical_records, summarize_reviews, CitedPassage,
    HistoricalFinding, HistoricalRecord, PlaceIdentity, PlacePrecision, CRITIC_ORIGIN,
};
use talaria_sources::{
    gazetteer_exact_identities, gazetteer_identities_for_qid, GazetteerIdentity,
};
use talaria_store::{
    list_active_events_for_qid, list_active_events_page, list_review_evidence, ReviewEventRow,
    ReviewEvidenceRow, TaskEffect,
};

pub const REVIEW_SAMPLE_LIMIT: i64 = 20;
pub const REVIEW_PAGE_SIZE: i64 = 100;
const PASSAGES_PER_EVENT: usize = 12;

struct EventView {
    title: String,
    event_type: String,
    time_json: Value,
    place_label: String,
    place_qid: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    evidence: Vec<Value>,
    passages_available: usize,
    passages_analyzed: usize,
    reliable_place_identity: bool,
    identity_count: usize,
    identity_precisions: Vec<String>,
}

pub async fn review_person(pool: &PgPool, qid: &str, limit: i64) -> anyhow::Result<Value> {
    let mut tx = begin_read_only(pool).await?;
    let rows = list_active_events_for_qid(&mut *tx, qid, limit).await?;
    let (records, views, fingerprint_parts) = collect_rows(&mut tx, &rows).await?;
    tx.commit().await?;
    let mut body = finish_report(records, views, fingerprint_parts);
    body["scope"] = json!("sample");
    Ok(body)
}

/// Every active person event, in stable pages. The sample limit does not apply.
pub async fn review_person_all(pool: &PgPool, qid: &str) -> anyhow::Result<Value> {
    let mut tx = begin_read_only(pool).await?;
    let mut records = Vec::new();
    let mut views = Vec::new();
    let mut fingerprint_parts = Vec::new();
    let mut offset = 0i64;
    loop {
        let rows = list_active_events_page(&mut *tx, qid, REVIEW_PAGE_SIZE, offset).await?;
        if rows.is_empty() {
            break;
        }
        let page_len = rows.len() as i64;
        let (page_records, page_views, mut page_parts) = collect_rows(&mut tx, &rows).await?;
        records.extend(page_records);
        views.extend(page_views);
        fingerprint_parts.append(&mut page_parts);
        offset += page_len;
        if page_len < REVIEW_PAGE_SIZE {
            break;
        }
    }
    tx.commit().await?;
    let mut body = finish_report(records, views, fingerprint_parts);
    body["scope"] = json!("all_active_person_events");
    Ok(body)
}

async fn collect_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rows: &[ReviewEventRow],
) -> anyhow::Result<(Vec<HistoricalRecord>, Vec<EventView>, Vec<String>)> {
    let mut records = Vec::with_capacity(rows.len());
    let mut views = Vec::with_capacity(rows.len());
    let mut fingerprint_parts = Vec::new();
    for row in rows {
        let evidence = list_review_evidence(&mut **tx, row.id).await?;
        for item in &evidence {
            fingerprint_parts.push(format!(
                "{}:{}",
                item.id,
                item.quoted_text.clone().unwrap_or_default()
            ));
        }
        let (record, view) = prepare_event(row, &evidence);
        records.push(record);
        views.push(view);
    }
    Ok((records, views, fingerprint_parts))
}

fn finish_report(
    records: Vec<HistoricalRecord>,
    views: Vec<EventView>,
    mut fingerprint_parts: Vec<String>,
) -> Value {
    let reviewed = review_historical_records(&records);
    let mut body = report_json(&records, &reviewed, &views);
    fingerprint_parts.sort();
    body["input_fingerprint"] = json!(fingerprint_of(&fingerprint_parts.join("\n")));
    body["detector_version"] = json!(talaria_ai::CRITIC_DETECTOR_VERSION);
    body
}

pub fn review_storage_key(qid: &str, detector_version: &str, input_fingerprint: &str) -> String {
    format!("review_historical_consistency:{qid}:{detector_version}:{input_fingerprint}")
}

async fn begin_read_only(pool: &PgPool) -> anyhow::Result<sqlx::Transaction<'_, sqlx::Postgres>> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub async fn review_task(pool: &PgPool, qid: &str) -> TaskEffect {
    match review_person_all(pool, qid).await {
        Ok(detail) => {
            let fingerprint = detail["input_fingerprint"].as_str().unwrap_or("none");
            let version = detail["detector_version"]
                .as_str()
                .unwrap_or(talaria_ai::CRITIC_DETECTOR_VERSION);
            TaskEffect {
                key: review_storage_key(qid, version, fingerprint),
                kind: "review_historical_consistency".into(),
                status: "completed",
                writes: false,
                detail,
            }
        }
        Err(error) => TaskEffect {
            key: review_storage_key(qid, talaria_ai::CRITIC_DETECTOR_VERSION, "failed"),
            kind: "review_historical_consistency".into(),
            status: "failed",
            writes: false,
            detail: json!({
                "reason": error.to_string(),
                "canonical_write": false
            }),
        },
    }
}

fn fingerprint_of(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub async fn content_fingerprint(pool: &PgPool) -> anyhow::Result<String> {
    let digest = sqlx::query_scalar::<_, String>(
        r#"
        SELECT md5(concat_ws(':',
            (SELECT COALESCE(string_agg(md5(concat_ws('|', id::text, event_type, title, time_json::text, coalesce(place_label, ''), coalesce(place_identity_qid, ''), coalesce(ST_AsText(geom), ''), is_active::text)), '' ORDER BY id::text), '') FROM canonical_events),
            (SELECT COALESCE(string_agg(md5(concat_ws('|', id::text, status, event_type, time_json::text, coalesce(place_label, ''))), '' ORDER BY id::text), '') FROM event_candidates),
            (SELECT COALESCE(string_agg(md5(concat_ws('|', id::text, canonical_event_id::text, coalesce(quoted_text, ''), coalesce(source_locator, ''))), '' ORDER BY id::text), '') FROM event_evidence)
        ))
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(digest)
}

pub async fn canon_counts(pool: &PgPool) -> anyhow::Result<(i64, i64, i64)> {
    let events = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
        .fetch_one(pool)
        .await?;
    let candidates = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM event_candidates")
        .fetch_one(pool)
        .await?;
    let evidence = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM event_evidence")
        .fetch_one(pool)
        .await?;
    Ok((events, candidates, evidence))
}

fn prepare_event(
    row: &ReviewEventRow,
    evidence: &[ReviewEvidenceRow],
) -> (HistoricalRecord, EventView) {
    let place_label = row.place_label.clone().unwrap_or_default();
    let identities = identities_for(row);
    let selected = select_passages(evidence, &place_label);
    let passages = selected
        .iter()
        .filter_map(|item| {
            let quote = item.quoted_text.clone().unwrap_or_default();
            if quote.trim().is_empty() {
                return None;
            }
            Some(CitedPassage {
                quote,
                url: evidence_url(item),
            })
        })
        .collect();
    let year = row
        .time_json
        .get("start")
        .and_then(Value::as_str)
        .and_then(year_from_start);
    let precision = row
        .time_json
        .get("precision")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let mut anchors = Vec::new();
    if row.title.trim().chars().count() >= 4 {
        anchors.push(row.title.clone());
    }
    if !place_label.trim().is_empty() {
        anchors.push(place_label.clone());
    }
    let record = HistoricalRecord {
        event_id: row.id.to_string(),
        title: row.title.clone(),
        event_type: row.event_type.clone(),
        anchors,
        stored_year: year,
        stored_precision: precision,
        place_label: place_label.clone(),
        place_qid: row.place_identity_qid.clone(),
        lat: row.lat,
        lon: row.lon,
        passages,
        identities,
        occurrence_key: row.occurrence_key.clone(),
    };
    let view = EventView {
        title: row.title.clone(),
        event_type: row.event_type.clone(),
        time_json: row.time_json.clone(),
        place_label,
        place_qid: row.place_identity_qid.clone(),
        lat: row.lat,
        lon: row.lon,
        evidence: evidence
            .iter()
            .map(|item| json!({"evidence_id": item.id, "url": evidence_url(item)}))
            .collect(),
        passages_available: evidence.len(),
        passages_analyzed: record.passages.len(),
        reliable_place_identity: !record.identities.is_empty(),
        identity_count: record.identities.len(),
        identity_precisions: record
            .identities
            .iter()
            .map(|identity| match identity.precision {
                PlacePrecision::Exact => "exact",
                PlacePrecision::Approximate => "approximate",
                PlacePrecision::Centroid => "centroid",
            })
            .map(str::to_string)
            .collect(),
    };
    (record, view)
}

fn identities_for(row: &ReviewEventRow) -> Vec<PlaceIdentity> {
    let found = if let Some(qid) = row
        .place_identity_qid
        .as_deref()
        .filter(|qid| !qid.trim().is_empty())
    {
        gazetteer_identities_for_qid(qid)
    } else {
        gazetteer_exact_identities(row.place_label.as_deref().unwrap_or(""))
    };
    found.into_iter().filter_map(to_critic_identity).collect()
}

fn to_critic_identity(identity: GazetteerIdentity) -> Option<PlaceIdentity> {
    Some(PlaceIdentity {
        qid: identity.qid.unwrap_or_default(),
        label: identity.label,
        lat: identity.lat,
        lon: identity.lon,
        precision: PlacePrecision::parse(&identity.precision)?,
        uncertainty_radius_m: identity.uncertainty_radius_m,
    })
}

fn select_passages(evidence: &[ReviewEvidenceRow], place_label: &str) -> Vec<ReviewEvidenceRow> {
    let place = place_label.to_ascii_lowercase();
    let mut ranked: Vec<ReviewEvidenceRow> = evidence.to_vec();
    ranked.sort_by_key(|row| {
        let quote = row
            .quoted_text
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if quote_has_year(&quote) || (!place.is_empty() && quote.contains(&place)) {
            0
        } else {
            1
        }
    });
    ranked.truncate(PASSAGES_PER_EVENT);
    ranked
}

fn quote_has_year(quote: &str) -> bool {
    let mut digits = 0;
    for character in quote.chars() {
        if character.is_ascii_digit() {
            digits += 1;
            if digits >= 3 {
                return true;
            }
        } else if digits > 0 {
            digits = 0;
        }
    }
    false
}

fn evidence_url(row: &ReviewEvidenceRow) -> String {
    if let Some(locator) = &row.source_locator {
        if let Ok(value) = serde_json::from_str::<Value>(locator) {
            let uri = value
                .get("uri")
                .or_else(|| value.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if uri.starts_with("https://") {
                return uri.to_string();
            }
        }
    }
    format!("evidence:{}", row.id)
}

fn year_from_start(start: &str) -> Option<i32> {
    let trimmed = start.trim();
    let negative = trimmed.starts_with('-');
    let digits: String = trimmed
        .trim_start_matches('-')
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let year: i32 = digits.parse().ok()?;
    Some(if negative { -year } else { year })
}

fn coverage_level(view: &EventView) -> &'static str {
    if view.passages_analyzed == 0 {
        "none"
    } else if view.passages_analyzed < view.passages_available
        || (!view.place_label.is_empty() && !view.reliable_place_identity)
    {
        "partial"
    } else {
        "bounded"
    }
}

fn report_json(
    records: &[HistoricalRecord],
    reviewed: &[Vec<HistoricalFinding>],
    views: &[EventView],
) -> Value {
    let summary = summarize_reviews(reviewed);
    let mut summary_json = summary.to_json();
    let mut none = 0;
    let mut partial = 0;
    let mut bounded = 0;
    let events = records
        .iter()
        .zip(views)
        .zip(reviewed)
        .map(|((record, view), findings)| {
            let level = coverage_level(view);
            match level {
                "none" => none += 1,
                "partial" => partial += 1,
                _ => bounded += 1,
            }
            json!({
                "event_id": record.event_id,
                "title": view.title,
                "event_type": view.event_type,
                "time": view.time_json,
                "place_label": view.place_label,
                "place_qid": view.place_qid,
                "lat": view.lat,
                "lon": view.lon,
                "evidence": view.evidence,
                "coverage": {
                    "passages_available": view.passages_available,
                    "passages_analyzed": view.passages_analyzed,
                    "reliable_place_identity": view.reliable_place_identity,
                    "level": level,
                    "certifies_historical_truth": false
                },
                "place_identities": {
                    "count": view.identity_count,
                    "precisions": view.identity_precisions
                },
                "findings": findings.iter().map(HistoricalFinding::to_json).collect::<Vec<_>>(),
                "status": exclusive_status(findings)
            })
        })
        .collect::<Vec<_>>();
    summary_json["coverage"] = json!({
        "events_without_passages": none,
        "events_partial": partial,
        "events_bounded": bounded,
        "no_anomaly_is_not_verification": true
    });
    json!({
        "origin": CRITIC_ORIGIN,
        "canonical_write": false,
        "model": {"policy": "deterministic", "calls": 0, "input_tokens": 0, "output_tokens": 0, "cost_usd_micros": 0},
        "summary": summary_json,
        "events": events
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_v3_does_not_reuse_a_v2_storage_key() {
        let fingerprint = "same-evidence";
        assert_eq!(talaria_ai::CRITIC_DETECTOR_VERSION, "place-role:v3");
        assert_ne!(
            review_storage_key("Q517", "place-role:v2", fingerprint),
            review_storage_key("Q517", talaria_ai::CRITIC_DETECTOR_VERSION, fingerprint)
        );
    }
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use talaria_store::{
        ensure_subject_entity, insert_person_event, insert_person_quote_evidence,
        record_supervisor_run, PersonEventInsert, SupervisorCall,
    };
    use uuid::Uuid;

    fn at_year(year: i32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, 6, 1, 0, 0, 0)
            .single()
            .expect("date")
    }

    fn event(
        entity_id: Uuid,
        title: &str,
        event_type: &str,
        year: i32,
        precision: &str,
        place: &str,
        qid: Option<&str>,
        lat: f64,
        lon: f64,
        key: &str,
    ) -> PersonEventInsert {
        PersonEventInsert {
            entity_id,
            event_type: event_type.into(),
            epistemic_status: "attested".into(),
            title: title.into(),
            summary: None,
            start_time: Some(at_year(year)),
            time_json: json!({
                "kind": "exact",
                "precision": precision,
                "start": year.to_string(),
                "surface": year.to_string(),
                "calendar": "gregorian"
            }),
            place_label: Some(place.into()),
            lat: Some(lat),
            lon: Some(lon),
            confidence: 0.8,
            map_eligible: true,
            fingerprint: format!("critic-{key}"),
            occurrence_key: key.into(),
            occurrence_stem: None,
            predicate: "direct".into(),
            place_identity_qid: qid.map(str::to_string),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_loaded_sample_is_reviewed_without_writing_history(pool: PgPool) {
        let entity_id = ensure_subject_entity(&pool, "Q900001", "Critic Subject")
            .await
            .unwrap();
        let valmy = insert_person_event(
            &pool,
            &event(
                entity_id,
                "Battle of Valmy",
                "battle",
                1792,
                "year",
                "Valmy",
                None,
                -33.40,
                -70.57,
                "valmy",
            ),
        )
        .await
        .unwrap();
        insert_person_quote_evidence(
            &pool,
            valmy,
            "The Battle of Valmy was fought in September 1792.",
            None,
            0.9,
            r#"{"uri":"https://en.wikipedia.org/wiki/Battle_of_Valmy"}"#,
        )
        .await
        .unwrap();
        insert_person_event(
            &pool,
            &event(
                entity_id,
                "Battle of Wagram",
                "battle",
                1780,
                "year",
                "Wagram",
                None,
                48.25,
                16.5667,
                "wagram",
            ),
        )
        .await
        .unwrap();
        let wagram =
            talaria_store::find_active_person_event_by_occurrence(&pool, entity_id, "wagram")
                .await
                .unwrap()
                .unwrap();
        insert_person_quote_evidence(
            &pool,
            wagram,
            "In 1809 the Treaty of Schönbrunn was signed in Vienna. The Battle of Wagram was costly.",
            None,
            0.9,
            r#"{"uri":"https://en.wikipedia.org/wiki/Battle_of_Wagram"}"#,
        )
        .await
        .unwrap();
        insert_person_event(
            &pool,
            &event(
                entity_id,
                "Unknown stop",
                "travel",
                1800,
                "year",
                "Nowhereville",
                None,
                10.0,
                10.0,
                "missing",
            ),
        )
        .await
        .unwrap();
        insert_person_event(
            &pool,
            &event(
                entity_id,
                "Battle of Lodi",
                "battle",
                1796,
                "day",
                "Lodi",
                None,
                45.314,
                9.503,
                "lodi",
            ),
        )
        .await
        .unwrap();
        let lodi = talaria_store::find_active_person_event_by_occurrence(&pool, entity_id, "lodi")
            .await
            .unwrap()
            .unwrap();
        insert_person_quote_evidence(
            &pool,
            lodi,
            "The Battle of Lodi was fought on 10 May 1796 at Lodi.",
            None,
            0.9,
            r#"{"uri":"https://en.wikipedia.org/wiki/Battle_of_Lodi"}"#,
        )
        .await
        .unwrap();
        insert_person_event(
            &pool,
            &event(
                entity_id,
                "Duke of Valmy",
                "office",
                1808,
                "year",
                "Duchy of Valmy",
                Some("Q649178"),
                -33.40,
                -70.57,
                "duchy",
            ),
        )
        .await
        .unwrap();

        let before_rows = content_fingerprint(&pool).await.unwrap();
        let before = canon_counts(&pool).await.unwrap();
        let report = review_person(&pool, "Q900001", REVIEW_SAMPLE_LIMIT)
            .await
            .unwrap();
        let after = canon_counts(&pool).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(content_fingerprint(&pool).await.unwrap(), before_rows);
        assert_eq!(report["canonical_write"], false);
        assert_eq!(
            report["summary"]["coverage"]["no_anomaly_is_not_verification"],
            true
        );
        assert_eq!(report["model"]["calls"], 0);
        assert_eq!(report["model"]["cost_usd_micros"], 0);
        assert_eq!(report["summary"]["examined"], 5);
        let events = report["events"].as_array().unwrap();
        let valmy_report = events
            .iter()
            .find(|event| event["title"] == "Battle of Valmy")
            .unwrap();
        assert_eq!(valmy_report["status"], "proposal");
        assert_eq!(valmy_report["coverage"]["level"], "bounded");
        assert_eq!(
            valmy_report["coverage"]["certifies_historical_truth"],
            false
        );
        assert_eq!(
            valmy_report["evidence"][0]["url"],
            "https://en.wikipedia.org/wiki/Battle_of_Valmy"
        );
        assert!(valmy_report["findings"][0]["proposal"]["source"]
            .as_str()
            .unwrap()
            .contains("Q649178"));
        let wagram_report = events
            .iter()
            .find(|event| event["title"] == "Battle of Wagram")
            .unwrap();
        assert_eq!(wagram_report["status"], "no_anomaly");
        assert_eq!(
            wagram_report["coverage"]["certifies_historical_truth"],
            false
        );
        let missing = events
            .iter()
            .find(|event| event["title"] == "Unknown stop")
            .unwrap();
        assert_eq!(missing["status"], "no_anomaly");
        assert_eq!(missing["coverage"]["level"], "none");
        assert!(missing["evidence"].as_array().unwrap().is_empty());
        let lodi_report = events
            .iter()
            .find(|event| event["title"] == "Battle of Lodi")
            .unwrap();
        assert_eq!(lodi_report["status"], "no_anomaly");
        let duchy = events
            .iter()
            .find(|event| event["title"] == "Duke of Valmy")
            .unwrap();
        assert_eq!(duchy["status"], "needs_review");
        assert_eq!(duchy["place_qid"], "Q649178");
        assert!(duchy["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["proposal"].is_null()));

        let mut tx = begin_read_only(&pool).await.unwrap();
        let denied = sqlx::query("UPDATE canonical_events SET title = title")
            .execute(&mut *tx)
            .await;
        assert!(denied.is_err());

        let effect = review_task(&pool, "Q900001").await;
        assert!(!effect.writes);
        let first_key = effect.key.clone();
        let call = SupervisorCall {
            experience: "istoria".into(),
            subject_qid: "Q900001".into(),
            subject_label: "Critic Subject".into(),
            provider: "deterministic".into(),
            model: "none".into(),
            prompt_version: "supervisor:v2".into(),
            plan_json: json!({"tasks": [{"key": effect.key}]}),
            execute: true,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd_micros: 0,
            cost_estimated: false,
            duration_ms: 0,
            effects: vec![effect],
            sourced_site: None,
        };
        record_supervisor_run(&pool, &call).await.unwrap();
        record_supervisor_run(&pool, &call).await.unwrap();
        let runs = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM supervisor_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        let tasks = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM supervisor_tasks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(runs, 1);
        assert_eq!(tasks, 1);
        assert_eq!(canon_counts(&pool).await.unwrap(), before);
        assert_eq!(content_fingerprint(&pool).await.unwrap(), before_rows);
        insert_person_quote_evidence(
            &pool,
            valmy,
            "A later archival note does not describe the battle.",
            None,
            0.4,
            r#"{"uri":"https://en.wikipedia.org/wiki/Battle_of_Valmy"}"#,
        )
        .await
        .unwrap();
        let after_new_evidence = content_fingerprint(&pool).await.unwrap();
        assert_ne!(after_new_evidence, before_rows);
        let changed = review_task(&pool, "Q900001").await;
        assert_ne!(changed.key, first_key);
        let second = SupervisorCall {
            plan_json: json!({"tasks": [{"key": changed.key}]}),
            effects: vec![changed],
            ..call
        };
        record_supervisor_run(&pool, &second).await.unwrap();
        let runs_after_change =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM supervisor_runs")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(runs_after_change, 2);
        assert_eq!(
            content_fingerprint(&pool).await.unwrap(),
            after_new_evidence
        );
    }

    #[tokio::test]
    #[ignore = "reads the local development database and writes nothing"]
    async fn read_only_napoleon_sample() {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("database");
        let before = canon_counts(&pool).await.unwrap();
        let report = review_person(&pool, "Q517", REVIEW_SAMPLE_LIMIT)
            .await
            .unwrap();
        let after = canon_counts(&pool).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(report["canonical_write"], false);
        println!(
            "{}",
            serde_json::to_string_pretty(&report["summary"]).unwrap()
        );
        if let Some(events) = report["events"].as_array() {
            for event in events {
                let findings = event["findings"].as_array();
                let problems = findings
                    .into_iter()
                    .flatten()
                    .map(|finding| {
                        let problem = finding["problem"].as_str().unwrap_or("");
                        let proposal = finding["proposal"].as_object();
                        let value = proposal
                            .and_then(|item| {
                                item.get("start")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .or_else(|| {
                                proposal.and_then(|item| {
                                    item.get("source")
                                        .and_then(Value::as_str)
                                        .map(str::to_string)
                                })
                            })
                            .unwrap_or_default();
                        if value.is_empty() {
                            problem.to_string()
                        } else {
                            format!("{problem}={value}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{} | {} | {} | {}",
                    event["status"].as_str().unwrap_or(""),
                    event["event_type"].as_str().unwrap_or(""),
                    event["title"].as_str().unwrap_or(""),
                    problems
                );
            }
        }
    }

    #[tokio::test]
    #[ignore = "reads the local development database and appends one supervisor journal row"]
    async fn read_only_napoleon_full_audit() {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .expect("database");
        let active = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT count(*)::bigint
            FROM canonical_events ce
            INNER JOIN entities e ON e.id = ce.entity_id
            WHERE e.qid = 'Q517' AND ce.is_active = true AND ce.pipeline = 'person'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let label = sqlx::query_scalar::<_, String>(
            "SELECT canonical_name FROM entities WHERE qid = 'Q517' ORDER BY created_at LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let before_rows = content_fingerprint(&pool).await.unwrap();
        let before = canon_counts(&pool).await.unwrap();
        let started = std::time::Instant::now();
        let report = review_person_all(&pool, "Q517").await.unwrap();
        let duration_ms = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
        let after = canon_counts(&pool).await.unwrap();
        let after_rows = content_fingerprint(&pool).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(before_rows, after_rows);
        assert_eq!(report["canonical_write"], false);
        assert_eq!(report["scope"], "all_active_person_events");
        assert_eq!(report["model"]["calls"], 0);
        assert_eq!(report["model"]["cost_usd_micros"], 0);
        assert_eq!(report["summary"]["examined"].as_i64().unwrap(), active);
        assert!(active > REVIEW_SAMPLE_LIMIT);
        let examined = report["summary"]["examined"].as_u64().unwrap();
        let quiet = report["summary"]["events_no_anomaly"].as_u64().unwrap();
        let proposed = report["summary"]["events_proposal"].as_u64().unwrap();
        let review = report["summary"]["events_needs_review"].as_u64().unwrap();
        let missing = report["summary"]["events_insufficient_evidence"]
            .as_u64()
            .unwrap();
        assert_eq!(quiet + proposed + review + missing, examined);
        let effect = TaskEffect {
            key: review_storage_key(
                "Q517",
                report["detector_version"].as_str().unwrap(),
                report["input_fingerprint"].as_str().unwrap(),
            ),
            kind: "review_historical_consistency".into(),
            status: "completed",
            writes: false,
            detail: report.clone(),
        };
        assert!(!effect.writes);
        let call = SupervisorCall {
            experience: "istoria".into(),
            subject_qid: "Q517".into(),
            subject_label: label,
            provider: "deterministic".into(),
            model: "none".into(),
            prompt_version: "supervisor:v2".into(),
            plan_json: json!({
                "tasks": [{"key": effect.key}],
                "scope": "all_active_person_events",
                "llm": false
            }),
            execute: false,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd_micros: 0,
            cost_estimated: false,
            duration_ms,
            effects: vec![effect],
            sourced_site: None,
        };
        let stored = record_supervisor_run(&pool, &call).await.unwrap();
        let again = record_supervisor_run(&pool, &call).await.unwrap();
        assert_eq!(stored["idempotent"], false);
        assert_eq!(again["idempotent"], true);
        assert_eq!(stored["run_id"], again["run_id"]);
        assert_eq!(stored["tasks"][0]["writes"], false);
        let run_id = stored["run_id"].as_str().unwrap();
        let tasks = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM supervisor_tasks WHERE run_id = $1::uuid",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tasks, 1);
        assert_eq!(canon_counts(&pool).await.unwrap(), before);
        assert_eq!(content_fingerprint(&pool).await.unwrap(), before_rows);
        let bundle = json!({
            "active_events": active,
            "duration_ms": duration_ms,
            "fingerprint_before": before_rows,
            "fingerprint_after": after_rows,
            "counts_before": {"canonical_events": before.0, "event_candidates": before.1, "event_evidence": before.2},
            "journal_run_id": run_id,
            "journal_idempotent_rerun": true,
            "report": report
        });
        std::fs::write(
            "/tmp/napoleon-q517-audit.json",
            serde_json::to_vec(&bundle).unwrap(),
        )
        .unwrap();
        println!("examined={active} duration_ms={duration_ms} run={run_id}");
    }

    #[tokio::test]
    #[ignore = "reads the local development database and does not write"]
    async fn read_only_napoleon_reevaluation() {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .expect("database");
        let before_rows = content_fingerprint(&pool).await.unwrap();
        let before = canon_counts(&pool).await.unwrap();
        let report = review_person_all(&pool, "Q517").await.unwrap();
        let after = canon_counts(&pool).await.unwrap();
        let after_rows = content_fingerprint(&pool).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(before_rows, after_rows);
        assert_eq!(report["canonical_write"], false);
        let bundle = json!({
            "fingerprint_before": before_rows,
            "fingerprint_after": after_rows,
            "counts_before": {"canonical_events": before.0, "event_candidates": before.1, "event_evidence": before.2},
            "journal_write": false,
            "report": report
        });
        std::fs::write(
            "/tmp/napoleon-q517-reaudit.json",
            serde_json::to_vec(&bundle).unwrap(),
        )
        .unwrap();
    }
}
