//! QID-based heritage discovery. Never writes canonical_events.
use super::AppState;
use axum::{extract::{Path, Query, State}, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct SitesQuery { pub bbox: Option<String>, pub limit: Option<i64> }

fn failure(status: StatusCode, code: &'static str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

fn bbox(input: Option<&str>) -> Result<Option<[f64;4]>, ()> {
    let Some(value) = input else { return Ok(None) };
    let nums = value.split(',').map(str::parse::<f64>)
        .collect::<Result<Vec<_>,_>>().map_err(|_| ())?;
    if nums.len()!=4 || nums.iter().any(|n| !n.is_finite()) ||
        nums[0] < -180.0 || nums[2] > 180.0 || nums[1] < -90.0 || nums[3] > 90.0 ||
        nums[0] > nums[2] || nums[1] > nums[3] { return Err(()) }
    Ok(Some([nums[0],nums[1],nums[2],nums[3]]))
}

pub async fn discover(State(state): State<AppState>, Path(entity_id): Path<Uuid>) -> Response {
    if state.offline_only { return failure(StatusCode::SERVICE_UNAVAILABLE, "offline_only"); }
    let row: Option<(Option<String>,)> = match sqlx::query_as("SELECT qid FROM entities WHERE id=$1")
        .bind(entity_id).fetch_optional(&state.pool).await {
        Ok(v) => v, Err(_) => return failure(StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
    };
    let Some((Some(qid),)) = row else { return failure(StatusCode::NOT_FOUND, "entity_without_qid") };
    if !valid_qid(&qid) { return failure(StatusCode::BAD_REQUEST, "invalid_qid") };
    let query = format!(r#"PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX bd: <http://www.bigdata.com/rdf#>
SELECT DISTINCT ?site ?siteLabel ?relation ?geo ?class WHERE {{
  {{ wd:{qid} wdt:P119 ?site . BIND("P119" AS ?relation) }}
  UNION {{ wd:{qid} wdt:P551 ?site . BIND("P551" AS ?relation) }}
  UNION {{ ?site wdt:P547 wd:{qid} . BIND("P547" AS ?relation) }}
  UNION {{ ?site wdt:P138 wd:{qid} . BIND("P138" AS ?relation) }}
  ?site wdt:P625 ?geo ; wdt:P31 ?class .
  FILTER EXISTS {{
    ?site wdt:P31/wdt:P279* ?allowed .
    VALUES ?allowed {{
      wd:Q41176 wd:Q33506 wd:Q4989906 wd:Q5003624 wd:Q811979
    }}
  }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "fr,en". }}
}} LIMIT 200"#);
    let client=match reqwest::Client::builder()
        .user_agent("Talaria/1.0 (heritage discovery; https://github.com/ThibaultL24/TalariaV2)")
        .timeout(std::time::Duration::from_secs(40)).build() {
        Ok(c)=>c, Err(_)=>return failure(StatusCode::INTERNAL_SERVER_ERROR,"http_client_error")
    };
    let response=match client.get("https://query.wikidata.org/sparql")
        .query(&[("query",query.as_str()),("format","json")]).send().await {
        Ok(r)=>r, Err(_)=>return failure(StatusCode::BAD_GATEWAY,"wikidata_unreachable")
    };
    if !response.status().is_success(){return failure(StatusCode::BAD_GATEWAY,"wikidata_error")}
    let data:Value=match response.json().await {
        Ok(v)=>v, Err(_)=>return failure(StatusCode::BAD_GATEWAY,"wikidata_invalid_response")
    };
    let mut accepted=0usize;
    let mut rejected=0usize;
    for binding in data.pointer("/results/bindings").and_then(Value::as_array).into_iter().flatten() {
        let field=|key:&str| binding.get(key).and_then(|v|v.get("value")).and_then(Value::as_str);
        let (Some(site_uri),Some(label),Some(pid),Some(wkt))=(field("site"),field("siteLabel"),field("relation"),field("geo")) else { rejected+=1;continue };
        let Some(site_qid)=site_uri.strip_prefix("http://www.wikidata.org/entity/").or_else(||site_uri.strip_prefix("https://www.wikidata.org/entity/")) else {rejected+=1;continue};
        if !valid_qid(site_qid) || label.trim().is_empty(){rejected+=1;continue}
        let Some((lat,lon))=parse_point(wkt) else {rejected+=1;continue};
        let kind = match field("class").unwrap_or("") {
            "http://www.wikidata.org/entity/Q33506" => "museum",
            "http://www.wikidata.org/entity/Q4989906" => "monument",
            "http://www.wikidata.org/entity/Q5003624" => "memorial",
            _ => "building",
        };
        let mut tx=match state.pool.begin().await { Ok(v)=>v,Err(_)=>return failure(StatusCode::INTERNAL_SERVER_ERROR,"database_error")};
        let site_id:Result<Uuid,_>=sqlx::query_scalar(r#"
            INSERT INTO visit_sites(wikidata_qid,label,kind,geom,retrieved_at)
            VALUES($1,$2,$3,ST_SetSRID(ST_MakePoint($4,$5),4326)::geography,NOW())
            ON CONFLICT(wikidata_qid) DO UPDATE
              SET label=EXCLUDED.label, kind=EXCLUDED.kind, updated_at=NOW(),
                  retrieved_at=NOW()
            RETURNING id"#)
            .bind(site_qid).bind(label).bind(kind).bind(lon).bind(lat)
            .fetch_one(&mut *tx).await;
        let Ok(site_id)=site_id else { let _=tx.rollback().await;rejected+=1;continue };
        let reference=format!("https://www.wikidata.org/wiki/{}",site_qid);
        let linked=sqlx::query(r#"
            INSERT INTO visit_person_sites(entity_id,site_id,relation_pid,source_url,retrieved_at)
            VALUES($1,$2,$3,$4,NOW())
            ON CONFLICT(entity_id,site_id,relation_pid)
            DO UPDATE SET retrieved_at=NOW()"#)
            .bind(entity_id).bind(site_id).bind(pid).bind(reference)
            .execute(&mut *tx).await;
        if linked.is_err() || tx.commit().await.is_err(){rejected+=1;continue}
        accepted+=1;
    }
    Json(json!({"accepted":accepted,"rejected":rejected,"source":"wikidata","note":"Sites persisted separately from canonical_events"})).into_response()
}

fn valid_qid(q:&str)->bool {
    q.strip_prefix('Q').is_some_and(|rest| !rest.is_empty() && !rest.starts_with('0') && rest.bytes().all(|b|b.is_ascii_digit()))
}
fn parse_point(s:&str)->Option<(f64,f64)> {
    let body=s.strip_prefix("Point(")?.strip_suffix(')')?;
    let mut nums=body.split_whitespace();
    let lon=nums.next()?.parse::<f64>().ok()?;
    let lat=nums.next()?.parse::<f64>().ok()?;
    if nums.next().is_some() || !lon.is_finite() || !lat.is_finite() || lon.abs()>180.0 || lat.abs()>90.0 {return None}
    Some((lat,lon))
}

pub async fn sites(State(state): State<AppState>,Path(entity_id): Path<Uuid>,Query(query): Query<SitesQuery>)->Response {
    let bbox=match bbox(query.bbox.as_deref()){Ok(v)=>v,Err(_)=>return failure(StatusCode::BAD_REQUEST,"invalid_bbox")};
    let limit=query.limit.unwrap_or(100).clamp(1,500);
    let rows=sqlx::query_as::<_,(Uuid,String,String,String,String,f64,f64)>(r#"
        SELECT s.id,s.wikidata_qid,s.label,s.kind,p.relation_pid,
               ST_Y(s.geom::geometry),ST_X(s.geom::geometry)
        FROM visit_person_sites p JOIN visit_sites s ON s.id=p.site_id
        WHERE p.entity_id=$1 AND s.publication_status='published'
          AND ($2::float8 IS NULL OR ST_Intersects(s.geom,ST_MakeEnvelope($2,$3,$4,$5,4326)::geography))
        ORDER BY s.label,s.wikidata_qid LIMIT $6
    "#)
    .bind(entity_id).bind(bbox.map(|b|b[0])).bind(bbox.map(|b|b[1]))
    .bind(bbox.map(|b|b[2])).bind(bbox.map(|b|b[3])).bind(limit)
    .fetch_all(&state.pool).await;
    match rows {
        Ok(rows)=>Json(json!({"sites":rows.iter().map(|(id,qid,label,kind,pid,lat,lon)|json!({
            "id":id,"wikidata_qid":qid,"label":label,"kind":kind,"relation_pid":pid,
            "coordinates":{"lat":lat,"lon":lon},"visitability":"unknown"
        })).collect::<Vec<_>>(),"count":rows.len()})).into_response(),
        Err(e)=>{tracing::error!(error=%e,"visit site query failed");failure(StatusCode::INTERNAL_SERVER_ERROR,"database_error")}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_non_qid(){assert!(!valid_qid("Q0"));assert!(!valid_qid("Q5 UNION"));assert!(valid_qid("Q535"));}
    #[test] fn parses_wkt(){assert_eq!(parse_point("Point(2.3 48.8)"),Some((48.8,2.3)));assert_eq!(parse_point("Point(400 50)"),None);}
    #[test] fn invalid_bboxes(){assert!(bbox(Some("2,48,1,49")).is_err());assert!(bbox(Some("a,b,c,d")).is_err());}
}

pub async fn sites_geojson(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<SitesQuery>,
) -> Response {
    let bbox=match bbox(query.bbox.as_deref()){Ok(v)=>v,Err(_)=>return failure(StatusCode::BAD_REQUEST,"invalid_bbox")};
    let limit=query.limit.unwrap_or(200).clamp(1,500);
    let rows=sqlx::query_as::<_,(Uuid,String,String,String,String,f64,f64)>(r#"
        SELECT s.id,s.wikidata_qid,s.label,s.kind,p.relation_pid,
               ST_Y(s.geom::geometry),ST_X(s.geom::geometry)
        FROM visit_person_sites p JOIN visit_sites s ON s.id=p.site_id
        WHERE p.entity_id=$1 AND s.publication_status='published'
          AND ($2::float8 IS NULL OR ST_Intersects(s.geom,ST_MakeEnvelope($2,$3,$4,$5,4326)::geography))
        ORDER BY s.label,s.wikidata_qid LIMIT $6
    "#)
    .bind(entity_id).bind(bbox.map(|b|b[0])).bind(bbox.map(|b|b[1]))
    .bind(bbox.map(|b|b[2])).bind(bbox.map(|b|b[3])).bind(limit)
    .fetch_all(&state.pool).await;
    match rows {
        Ok(rows)=>{
            let features:Vec<Value>=rows.iter().map(|(id,qid,label,kind,pid,lat,lon)| json!({
                "type":"Feature","id":id,
                "geometry":{"type":"Point","coordinates":[lon,lat]},
                "properties":{"id":id,"title":label,"event_type":"museum",
                  "visit_layer":"heritage_site","kind":kind,"wikidata_qid":qid,"relation_pid":pid}
            })).collect();
            Json(json!({"type":"FeatureCollection","features":features,"count":features.len()})).into_response()
        },
        Err(e)=>{tracing::error!(error=%e,"visit geojson query failed");failure(StatusCode::INTERNAL_SERVER_ERROR,"database_error")}
    }
}

pub async fn refresh_osm(
    State(state): State<AppState>,
    Path(site_id): Path<Uuid>,
) -> Response {
    if state.offline_only { return failure(StatusCode::SERVICE_UNAVAILABLE,"offline_only") }
    let row:Option<(String,)> = match sqlx::query_as("SELECT wikidata_qid FROM visit_sites WHERE id=$1")
        .bind(site_id).fetch_optional(&state.pool).await {
        Ok(r)=>r,Err(_)=>return failure(StatusCode::INTERNAL_SERVER_ERROR,"database_error")
    };
    let Some((qid,))=row else { return failure(StatusCode::NOT_FOUND,"site_not_found") };
    if !valid_qid(&qid){return failure(StatusCode::BAD_REQUEST,"invalid_qid")}
    let overpass_query=format!(
        "[out:json][timeout:20];nwr[\"wikidata\"=\"{qid}\"][\"opening_hours\"];out tags 10;"
    );
    let client=match reqwest::Client::builder()
        .user_agent("Talaria/1.0 (OpenStreetMap exact-QID hours; https://github.com/ThibaultL24/TalariaV2)")
        .timeout(std::time::Duration::from_secs(35)).build() {
        Ok(c)=>c, Err(_)=>return failure(StatusCode::INTERNAL_SERVER_ERROR,"http_client_error")
    };
    let response=match client.post("https://overpass-api.de/api/interpreter")
        .form(&[("data",overpass_query)])
        .send().await {
            Ok(r)=>r, Err(_)=>return failure(StatusCode::BAD_GATEWAY,"osm_unreachable")
        };
    if !response.status().is_success(){return failure(StatusCode::BAD_GATEWAY,"osm_error")}
    let payload:Value=match response.json().await {
        Ok(v)=>v, Err(_)=>return failure(StatusCode::BAD_GATEWAY,"osm_invalid_response")
    };
    let matches=payload.get("elements").and_then(Value::as_array).into_iter().flatten()
        .filter_map(|v|{
            let tags=v.get("tags")?;
            if tags.get("wikidata")?.as_str()? != qid {return None}
            let hours=tags.get("opening_hours")?.as_str()?.trim();
            let element_type=v.get("type")?.as_str()?;
            let element_id=v.get("id")?.as_i64()?;
            if hours.is_empty() || hours.len()>512 || element_id<=0 ||
                !matches!(element_type,"node"|"way"|"relation") {return None}
            Some((element_type.to_string(),element_id,hours.to_string()))
        }).collect::<Vec<_>>();
    // Ambiguous matching OSM objects must be reviewed, not merged automatically.
    if matches.len()!=1 {
        return Json(json!({"status":"unverified","matches":matches.len(),"opening_hours":null})).into_response()
    }
    let (element_type,element_id,hours)=&matches[0];
    let result=sqlx::query(r#"
        UPDATE visit_sites
        SET osm_opening_hours=$2, osm_object_type=$3, osm_object_id=$4,
            osm_checked_at=NOW(), updated_at=NOW()
        WHERE id=$1 AND wikidata_qid=$5
    "#).bind(site_id).bind(hours).bind(element_type).bind(element_id).bind(qid)
        .execute(&state.pool).await;
    match result {
        Ok(_) => Json(json!({"status":"found","opening_hours":hours,
            "source":"openstreetmap","osm_object_type":element_type,
            "osm_object_id":element_id,"open_now":null})).into_response(),
        Err(_) =>failure(StatusCode::INTERNAL_SERVER_ERROR,"database_error")
    }
}
