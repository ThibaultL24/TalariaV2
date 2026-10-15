//! Destination lookup is a user-triggered search, not autocomplete.
//! Coordinates come from Wikidata P625 and do not create Talaria places.
use super::AppState;
use axum::{extract::{Query, State}, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::{HashMap,HashSet}, sync::{Mutex,OnceLock}, time::{Duration,Instant}};

#[derive(Deserialize)]
pub struct DestinationQuery { q: String, lang: Option<String> }

fn error(status:StatusCode, code:&str)->Response {
    (status,Json(json!({"error":code}))).into_response()
}

fn valid_query(q:&str)->bool {
    let chars=q.chars().count();
    (3..=80).contains(&chars) && !q.chars().any(char::is_control)
}

fn valid_qid(q:&str)->bool {
    q.strip_prefix('Q').is_some_and(|tail| !tail.is_empty() && !tail.starts_with('0') && tail.bytes().all(|b| b.is_ascii_digit()))
}

fn is_city(entity:&Value)->bool {
    const CLASSES:&[&str]=&[
        "Q515",      // city
        "Q3957",     // town
        "Q532",      // village
        "Q15284",    // municipality
        "Q484170",   // commune of France
        "Q486972",   // human settlement
        "Q5119",     // capital city
        "Q1549591",  // big city
        "Q747074",   // commune of Italy
    ];
    entity.pointer("/claims/P31").and_then(Value::as_array).is_some_and(|claims|
        claims.iter().any(|claim|claim.pointer("/mainsnak/datavalue/value/id")
            .and_then(Value::as_str).is_some_and(|qid|CLASSES.contains(&qid))))
}

fn coords(entity:&Value)->Option<(f64,f64)> {
    let statements=entity.pointer("/claims/P625")?.as_array()?;
    for statement in statements {
        let val=statement.pointer("/mainsnak/datavalue/value")?;
        let lat=val.get("latitude")?.as_f64()?;
        let lon=val.get("longitude")?.as_f64()?;
        if lat.is_finite() && lon.is_finite() && (-90.0..=90.0).contains(&lat)
            && (-180.0..=180.0).contains(&lon) {return Some((lat,lon))}
    }
    None
}

type Cached=(Instant,Value);
static CACHE:OnceLock<Mutex<HashMap<String,Cached>>>=OnceLock::new();

fn cache_get(key:&str)->Option<Value> {
    let cache=CACHE.get_or_init(||Mutex::new(HashMap::new()));
    let guard=cache.lock().ok()?;
    guard.get(key).and_then(|(at,val)| (at.elapsed()<Duration::from_secs(86400)).then(||val.clone()))
}
fn cache_put(key:String,value:Value){
    let cache=CACHE.get_or_init(||Mutex::new(HashMap::new()));
    if let Ok(mut guard)=cache.lock(){
        if guard.len()>300 {guard.clear();}
        guard.insert(key,(Instant::now(),value));
    }
}

pub async fn search(
    State(state):State<AppState>,
    Query(query):Query<DestinationQuery>,
)->Response {
    let q=query.q.trim();
    if !valid_query(q){return error(StatusCode::BAD_REQUEST,"invalid_destination_query")}
    let lang=match query.lang.as_deref(){Some("en")=>"en",_=>"fr"};
    let key=format!("{}:{}",lang,q.to_lowercase());
    if let Some(data)=cache_get(&key){return Json(data).into_response()}
    if state.offline_only {return error(StatusCode::SERVICE_UNAVAILABLE,"offline_only")}
    let client=match reqwest::Client::builder()
        .user_agent("Talaria/1.0 destination discovery (https://github.com/ThibaultL24/TalariaV2)")
        .timeout(Duration::from_secs(15)).build() {
        Ok(client)=>client,Err(_)=>return error(StatusCode::INTERNAL_SERVER_ERROR,"http_client_error")
    };
    let search=client.get("https://www.wikidata.org/w/api.php")
        .query(&[("action","wbsearchentities"),("search",q),("language",lang),
            ("uselang",lang),("type","item"),("limit","12"),("format","json")])
        .send().await;
    let Ok(resp)=search else {return error(StatusCode::BAD_GATEWAY,"wikidata_unreachable")};
    if !resp.status().is_success(){return error(StatusCode::BAD_GATEWAY,"wikidata_search_error")}
    let Ok(payload)=resp.bytes().await else {return error(StatusCode::BAD_GATEWAY,"wikidata_search_error")};
    if payload.len()>512_000 {return error(StatusCode::BAD_GATEWAY,"wikidata_response_too_large")}
    let Ok(search_json)=serde_json::from_slice::<Value>(&payload) else {
        return error(StatusCode::BAD_GATEWAY,"wikidata_invalid_response")
    };
    let hits=search_json.get("search").and_then(Value::as_array).cloned().unwrap_or_default();
    let qids:Vec<String>=hits.iter().filter_map(|hit|hit.get("id").and_then(Value::as_str))
        .filter(|qid|valid_qid(qid)).take(12).map(str::to_string).collect();
    if qids.is_empty(){
        let result=json!({"items":[],"source":"wikidata","query":q});
        cache_put(key,result.clone());return Json(result).into_response()
    }
    let ids = qids.join("|");
    let detail=client.get("https://www.wikidata.org/w/api.php")
        .query(&[("action","wbgetentities"),("ids",ids.as_str()),
            ("props","claims"),("format","json")]).send().await;
    let Ok(resp)=detail else {return error(StatusCode::BAD_GATEWAY,"wikidata_unreachable")};
    if !resp.status().is_success(){return error(StatusCode::BAD_GATEWAY,"wikidata_entity_error")}
    let Ok(bytes)=resp.bytes().await else {return error(StatusCode::BAD_GATEWAY,"wikidata_entity_error")};
    if bytes.len()>2_000_000 {return error(StatusCode::BAD_GATEWAY,"wikidata_response_too_large")}
    let Ok(detail_json)=serde_json::from_slice::<Value>(&bytes) else {
        return error(StatusCode::BAD_GATEWAY,"wikidata_invalid_response")
    };
    let mut seen=HashSet::new();
    let mut items=Vec::new();
    for hit in hits {
        let Some(qid)=hit.get("id").and_then(Value::as_str) else {continue};
        if !valid_qid(qid) || !seen.insert(qid.to_string()){continue}
        let Some(entity)=detail_json.get("entities").and_then(|v|v.get(qid)) else {continue};
        if !is_city(entity) {continue}
        let Some((lat,lon))=coords(entity) else {continue};
        let label=hit.get("label").and_then(Value::as_str).unwrap_or(qid);
        let description=hit.get("description").and_then(Value::as_str).unwrap_or("");
        items.push(json!({"qid":qid,"label":label,"description":description,
            "coordinates":{"lat":lat,"lon":lon},
            "source_url":format!("https://www.wikidata.org/wiki/{qid}")}));
        if items.len()>=8 {break}
    }
    let result=json!({"items":items,"source":"wikidata","query":q});
    cache_put(key,result.clone());
    Json(result).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn bounded_query_and_qid(){
        assert!(!valid_query("Pa"));assert!(valid_query("Paris"));
        assert!(!valid_qid("Q0"));assert!(valid_qid("Q90"));
    }
    #[test] fn requires_settlement_type_and_coordinates(){
        let entity=json!({"claims":{
          "P31":[{"mainsnak":{"datavalue":{"value":{"id":"Q484170"}}}}],
          "P625":[{"mainsnak":{"datavalue":{"value":{"latitude":48.8566,"longitude":2.3522}}}}]
        }});
        assert!(is_city(&entity));assert_eq!(coords(&entity),Some((48.8566,2.3522)));
        assert!(!is_city(&json!({"claims":{"P31":[]}})));
    }
}
