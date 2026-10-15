//! Lead-supported selection of existing canonical person events. No new event is inferred.
use super::AppState;
use axum::{extract::{Path, Query, State}, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

const VERSION: i16 = 1;

#[derive(Debug, Deserialize, Default)]
pub struct LandmarkQuery { limit: Option<i64> }

fn fail(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}

/// Refresh only an editorial projection. Reuses existing event evidence and locally
/// stored Wikipedia Lead; no LLM, no canonical insert/update.
pub async fn refresh(State(state): State<AppState>, Path(entity_id): Path<Uuid>) -> Response {
    if state.offline_only {
        // Local material is sufficient even in offline mode.
    }
    let mut tx=match state.pool.begin().await {
        Ok(tx)=>tx,Err(_)=>return fail(StatusCode::INTERNAL_SERVER_ERROR,"database_error")
    };
    let deleted=sqlx::query("DELETE FROM entity_landmark_mentions WHERE entity_id=$1 AND algorithm_version=$2")
        .bind(entity_id).bind(VERSION).execute(&mut *tx).await;
    if deleted.is_err(){return fail(StatusCode::INTERNAL_SERVER_ERROR,"database_error")}
    let added=sqlx::query(r#"
        WITH lead_page AS (
          SELECT wp.id, wp.revision_id, ws.text AS lead_text
          FROM entities en
          JOIN wiki_pages wp ON wp.title = en.wikipedia_title
                            AND wp.wiki_lang = en.wiki_lang
          JOIN wiki_sections ws ON ws.wiki_page_id = wp.id
                              AND ws.ordinal = 0 AND ws.title = 'Lead'
          WHERE en.id = $1
            AND length(ws.text) > 40
        ), eligible AS (
          SELECT ce.id AS event_id, ee.id AS evidence_id, s.id AS sentence_id,
                 lp.id AS wiki_page_id, lp.revision_id,
                 (CASE ce.event_type
                     WHEN 'battle' THEN 86 WHEN 'office' THEN 82
                     WHEN 'publication' THEN 80 WHEN 'exile' THEN 79
                     WHEN 'diplomatic' THEN 76 WHEN 'award' THEN 74
                     WHEN 'birth' THEN 18 WHEN 'death' THEN 18
                     ELSE 65 END
                  + LEAST(12, GREATEST(0, ce.evidence_count)))::smallint AS score
          FROM canonical_events ce
          JOIN event_evidence ee ON ee.canonical_event_id=ce.id
          JOIN sentences s ON s.id=ee.sentence_id
          JOIN lead_page lp ON lp.id=s.wiki_page_id
          WHERE ce.entity_id=$1 AND ce.pipeline='person'
            AND ce.is_active AND ce.timeline_eligible
            AND ce.event_type NOT IN ('museum','memorial','statue','street_naming','commemoration')
            AND length(btrim(s.text)) BETWEEN 38 AND 800
            AND position(lower(btrim(s.text)) IN lower(lp.lead_text)) > 0
        )
        INSERT INTO entity_landmark_mentions
            (entity_id, canonical_event_id, event_evidence_id, sentence_id,
             wiki_page_id, wikipedia_revision_id, algorithm_version, importance_score)
        SELECT $1,event_id,evidence_id,sentence_id,wiki_page_id,revision_id,$2,LEAST(100,score)
        FROM eligible
        ON CONFLICT DO NOTHING
    "#).bind(entity_id).bind(VERSION).execute(&mut *tx).await;
    let Ok(added)=added else {return fail(StatusCode::INTERNAL_SERVER_ERROR,"database_error")};
    if tx.commit().await.is_err(){return fail(StatusCode::INTERNAL_SERVER_ERROR,"database_error")}
    Json(json!({"status":"refreshed","mentions":added.rows_affected(),
        "algorithm_version":VERSION,"source":"wikipedia_lead_existing_event_evidence"})).into_response()
}

pub async fn list(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<LandmarkQuery>,
) -> Response {
    let limit=query.limit.unwrap_or(12);
    if !(1..=50).contains(&limit) {return fail(StatusCode::BAD_REQUEST,"invalid_limit")}
    let rows=sqlx::query_as::<_,(Uuid,String,String,String,serde_json::Value,String,String,Option<i64>,i16)>(r#"
        WITH best AS (
          SELECT DISTINCT ON (lm.canonical_event_id)
             lm.canonical_event_id,lm.importance_score,lm.sentence_id,
             lm.wikipedia_revision_id
          FROM entity_landmark_mentions lm
          WHERE lm.entity_id=$1 AND lm.algorithm_version=$2
          ORDER BY lm.canonical_event_id,lm.importance_score DESC,lm.sentence_id
        )
        SELECT ce.id,ce.title,ce.event_type,ce.epistemic_status,
               ce.time_json,s.text,wp.title,b.wikipedia_revision_id,b.importance_score
        FROM best b JOIN canonical_events ce ON ce.id=b.canonical_event_id
        JOIN sentences s ON s.id=b.sentence_id
        JOIN wiki_pages wp ON wp.id=s.wiki_page_id
        WHERE ce.is_active AND ce.pipeline='person' AND ce.timeline_eligible
        ORDER BY b.importance_score DESC,ce.start_time ASC NULLS LAST,ce.id
        LIMIT $3
    "#).bind(entity_id).bind(VERSION).bind(limit).fetch_all(&state.pool).await;
    match rows {
        Ok(rows)=>Json(json!({"count":rows.len(),"selection":"wikipedia_lead_evidence",
            "algorithm_version":VERSION,
            "landmarks":rows.into_iter().map(|(id,title,event_type,epistemic_status,time,quote,page,revision,score)|json!({
                "event_id":id,"title":title,"event_type":event_type,
                "epistemic_status":epistemic_status,"time":time,
                "importance_score":score,"lead_evidence":{
                    "quote":quote,"wikipedia_page":page,
                    "revision_id":revision,
                    "source_url":match revision {
                        Some(rev)=>format!("https://en.wikipedia.org/w/index.php?title={}&oldid={}",page.replace(' ',"_"),rev),
                        None=>format!("https://en.wikipedia.org/wiki/{}",page.replace(' ',"_"))
                    }
                }
            })).collect::<Vec<_>>()})).into_response(),
        Err(e)=>{tracing::error!(error=%e,"landmark list failed");fail(StatusCode::INTERNAL_SERVER_ERROR,"database_error")}
    }
}
