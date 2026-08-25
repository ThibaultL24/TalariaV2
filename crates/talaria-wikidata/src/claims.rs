// crates/talaria-wikidata/src/claims.rs
//! Parse Wikibase entity JSON into persistable statements and derived STATEMENT lines.

use serde_json::{json, Value};

use crate::promote::promote_event;
use crate::time::parse_wikibase_time;

const IDENTITY_PIDS: &[&str] = &["P569", "P570", "P19", "P20"];
const DATE_QUALIFIERS: &[&str] = &["P580", "P582", "P585"];

#[derive(Debug, Clone, PartialEq)]
pub struct StatementInsert {
    pub qid: String,
    pub guid: String,
    pub property: String,
    pub rank: String,
    pub snaktype: String,
    pub value_json: Value,
    pub qualifiers_json: Value,
    pub references_json: Value,
    pub revision_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedStatement {
    pub insert: StatementInsert,
    pub event: Option<(String, String, Option<i32>, Option<String>)>,
}

pub fn parse_entity_claims(entity: &Value) -> Vec<ParsedStatement> {
    let qid = entity
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let revision_id = entity_revision_id(entity);
    let Some(claims) = entity.get("claims").and_then(|c| c.as_object()) else {
        return Vec::new();
    };

    let mut parsed = Vec::new();
    for (pid, arr) in claims {
        let Some(stmts) = arr.as_array() else {
            continue;
        };
        for stmt in stmts {
            if let Some(row) = parse_statement(&qid, pid, revision_id.clone(), stmt) {
                parsed.push(row);
            }
        }
    }
    suppress_identity_normal_events(&mut parsed);
    parsed
}

/// Active identity year (P569/P570): preferred beats normal; deprecated is excluded.
pub fn identity_year(parsed: &[ParsedStatement], property: &str) -> Option<i32> {
    parsed.iter().find_map(|stmt| {
        if stmt.insert.property != property || stmt.insert.rank == "deprecated" {
            return None;
        }
        stmt.event.as_ref()?.2
    })
}

pub fn promoted_statement_lines(parsed: &[ParsedStatement]) -> String {
    let mut lines = Vec::new();
    for stmt in parsed {
        if stmt.insert.rank == "deprecated" {
            continue;
        }
        let Some((event_type, predicate, year, place_qid)) = &stmt.event else {
            continue;
        };
        let year = year.map(|y| y.to_string()).unwrap_or_default();
        let place = place_qid.as_deref().unwrap_or("");
        lines.push(format!(
            "STATEMENT\t{event_type}\t{predicate}\t{year}\t{place}"
        ));
    }
    lines.join("\n")
}

fn parse_statement(
    qid: &str,
    claims_pid: &str,
    revision_id: Option<String>,
    stmt: &Value,
) -> Option<ParsedStatement> {
    let guid = stmt.get("id").and_then(|v| v.as_str())?.to_string();
    let mainsnak = stmt.get("mainsnak").unwrap_or(&Value::Null);
    let property = mainsnak
        .get("property")
        .and_then(|v| v.as_str())
        .unwrap_or(claims_pid)
        .to_string();
    let rank = stmt
        .get("rank")
        .and_then(|v| v.as_str())
        .unwrap_or("normal")
        .to_string();
    let snaktype = mainsnak
        .get("snaktype")
        .and_then(|v| v.as_str())
        .unwrap_or("value")
        .to_string();
    let value_json = mainsnak
        .pointer("/datavalue/value")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let qualifiers_json = stmt.get("qualifiers").cloned().unwrap_or_else(|| json!({}));
    let references_json = stmt.get("references").cloned().unwrap_or_else(|| json!([]));

    let event = if snaktype == "value" {
        let has_date = statement_has_date(stmt);
        promote_event(&property, has_date, true).map(|(event_type, predicate)| {
            (
                event_type.to_string(),
                predicate.to_string(),
                event_year(stmt),
                statement_place_qid(stmt),
            )
        })
    } else {
        None
    };

    Some(ParsedStatement {
        insert: StatementInsert {
            qid: qid.to_string(),
            guid,
            property,
            rank,
            snaktype,
            value_json,
            qualifiers_json,
            references_json,
            revision_id,
        },
        event,
    })
}

fn suppress_identity_normal_events(parsed: &mut [ParsedStatement]) {
    for pid in IDENTITY_PIDS {
        let has_preferred = parsed
            .iter()
            .any(|s| s.insert.property == *pid && s.insert.rank == "preferred");
        if !has_preferred {
            continue;
        }
        for stmt in parsed.iter_mut() {
            if stmt.insert.property == *pid && stmt.insert.rank == "normal" {
                stmt.event = None;
            }
        }
    }
}

fn statement_has_date(stmt: &Value) -> bool {
    snak_time(stmt.get("mainsnak").unwrap_or(&Value::Null)).is_some()
        || DATE_QUALIFIERS
            .iter()
            .any(|pid| qualifier_time(stmt, pid).is_some())
}

fn event_year(stmt: &Value) -> Option<i32> {
    qualifier_time(stmt, "P580")
        .or_else(|| qualifier_time(stmt, "P582"))
        .or_else(|| qualifier_time(stmt, "P585"))
        .or_else(|| snak_time(stmt.get("mainsnak").unwrap_or(&Value::Null)))
        .map(|t| t.year)
}

fn statement_place_qid(stmt: &Value) -> Option<String> {
    qualifier_item_id(stmt, "P276").or_else(|| {
        stmt.pointer("/mainsnak/datavalue/value/id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

fn qualifier_time(stmt: &Value, pid: &str) -> Option<crate::time::WikibaseTime> {
    let snak = stmt.pointer(&format!("/qualifiers/{pid}/0"))?;
    snak_time(snak)
}

fn qualifier_item_id(stmt: &Value, pid: &str) -> Option<String> {
    stmt.pointer(&format!("/qualifiers/{pid}/0/datavalue/value/id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn snak_time(snak: &Value) -> Option<crate::time::WikibaseTime> {
    let time = snak.pointer("/datavalue/value/time")?.as_str()?;
    let precision = snak
        .pointer("/datavalue/value/precision")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32);
    let calendar = snak
        .pointer("/datavalue/value/calendarmodel")
        .and_then(|v| v.as_str());
    parse_wikibase_time(time, precision, calendar)
}

fn entity_revision_id(entity: &Value) -> Option<String> {
    let v = entity.get("lastrevid")?;
    if let Some(n) = v.as_u64() {
        return Some(n.to_string());
    }
    if let Some(n) = v.as_i64() {
        return Some(n.to_string());
    }
    v.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mini_q517() -> Value {
        json!({
            "id": "Q517",
            "lastrevid": 123,
            "claims": {
                "P551": [{
                    "id": "Q517$p551",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P551",
                        "datavalue": {
                            "value": { "entity-type": "item", "id": "Q90" },
                            "type": "wikibase-entityid"
                        }
                    },
                    "qualifiers": {
                        "P580": [{
                            "snaktype": "value",
                            "property": "P580",
                            "datavalue": {
                                "value": {
                                    "time": "+1804-12-02T00:00:00Z",
                                    "precision": 11,
                                    "calendarmodel": "http://www.wikidata.org/entity/Q1985727"
                                },
                                "type": "time"
                            }
                        }],
                        "P582": [{
                            "snaktype": "value",
                            "property": "P582",
                            "datavalue": {
                                "value": {
                                    "time": "+1814-04-06T00:00:00Z",
                                    "precision": 11
                                },
                                "type": "time"
                            }
                        }]
                    },
                    "references": []
                }],
                "P106": [{
                    "id": "Q517$p106",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P106",
                        "datavalue": {
                            "value": { "entity-type": "item", "id": "Q82955" },
                            "type": "wikibase-entityid"
                        }
                    }
                }]
            }
        })
    }

    #[test]
    fn q517_p551_promotes_residence_p106_stays_claim() {
        let parsed = parse_entity_claims(&mini_q517());
        assert_eq!(parsed.len(), 2);

        let p551 = parsed
            .iter()
            .find(|s| s.insert.property == "P551")
            .expect("P551 stored");
        let p106 = parsed
            .iter()
            .find(|s| s.insert.property == "P106")
            .expect("P106 stored");

        assert_eq!(p551.insert.qid, "Q517");
        assert_eq!(p551.insert.guid, "Q517$p551");
        assert_eq!(p551.insert.rank, "normal");
        assert_eq!(p551.insert.snaktype, "value");
        assert_eq!(p551.insert.revision_id.as_deref(), Some("123"));
        assert_eq!(p551.insert.value_json["id"], "Q90");
        assert!(p551.insert.qualifiers_json.get("P580").is_some());
        assert_eq!(
            p551.event,
            Some((
                "residence".into(),
                "resided_in".into(),
                Some(1804),
                Some("Q90".into())
            ))
        );

        assert_eq!(p106.insert.qid, "Q517");
        assert_eq!(p106.insert.guid, "Q517$p106");
        assert_eq!(p106.insert.revision_id.as_deref(), Some("123"));
        assert!(p106.event.is_none());

        let lines = promoted_statement_lines(&parsed);
        assert_eq!(lines, "STATEMENT\tresidence\tresided_in\t1804\tQ90");
    }

    #[test]
    fn somevalue_and_novalue_store_without_event() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 1,
            "claims": {
                "P569": [{
                    "id": "Q1$some",
                    "rank": "normal",
                    "mainsnak": { "snaktype": "somevalue", "property": "P569" }
                }],
                "P570": [{
                    "id": "Q1$no",
                    "rank": "normal",
                    "mainsnak": { "snaktype": "novalue", "property": "P570" }
                }]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|s| s.event.is_none()));
        assert_eq!(
            parsed
                .iter()
                .find(|s| s.insert.guid == "Q1$some")
                .unwrap()
                .insert
                .snaktype,
            "somevalue"
        );
        assert_eq!(
            parsed
                .iter()
                .find(|s| s.insert.guid == "Q1$no")
                .unwrap()
                .insert
                .snaktype,
            "novalue"
        );
        assert!(promoted_statement_lines(&parsed).is_empty());
    }

    #[test]
    fn deprecated_rank_skipped_in_promoted_lines() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 9,
            "claims": {
                "P551": [
                    {
                        "id": "Q1$old",
                        "rank": "deprecated",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P551",
                            "datavalue": { "value": { "id": "Q90" } }
                        },
                        "qualifiers": {
                            "P580": [{
                                "datavalue": { "value": { "time": "+1800-01-01T00:00:00Z", "precision": 9 } }
                            }]
                        }
                    },
                    {
                        "id": "Q1$ok",
                        "rank": "normal",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P551",
                            "datavalue": { "value": { "id": "Q90" } }
                        },
                        "qualifiers": {
                            "P580": [{
                                "datavalue": { "value": { "time": "+1804-01-01T00:00:00Z", "precision": 9 } }
                            }]
                        }
                    }
                ]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().any(|s| s.insert.rank == "deprecated" && s.event.is_some()));
        assert_eq!(
            promoted_statement_lines(&parsed),
            "STATEMENT\tresidence\tresided_in\t1804\tQ90"
        );
    }

    #[test]
    fn identity_preferred_hides_normal_in_projection() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 2,
            "claims": {
                "P569": [
                    {
                        "id": "Q1$pref",
                        "rank": "preferred",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P569",
                            "datavalue": {
                                "value": { "time": "+1769-08-15T00:00:00Z", "precision": 11 }
                            }
                        }
                    },
                    {
                        "id": "Q1$norm",
                        "rank": "normal",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P569",
                            "datavalue": {
                                "value": { "time": "+1768-01-01T00:00:00Z", "precision": 9 }
                            }
                        }
                    }
                ]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(parsed.len(), 2);
        let pref = parsed.iter().find(|s| s.insert.rank == "preferred").unwrap();
        let normal = parsed.iter().find(|s| s.insert.rank == "normal").unwrap();
        assert_eq!(
            pref.event,
            Some(("birth".into(), "born_in".into(), Some(1769), None))
        );
        assert!(normal.event.is_none());
        assert_eq!(
            promoted_statement_lines(&parsed),
            "STATEMENT\tbirth\tborn_in\t1769\t"
        );
        assert_eq!(identity_year(&parsed, "P569"), Some(1769));
    }

    #[test]
    fn preferred_p569_wins_lifespan_year_over_normal() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 2,
            "claims": {
                "P569": [
                    {
                        "id": "Q1$norm",
                        "rank": "normal",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P569",
                            "datavalue": {
                                "value": { "time": "+1768-01-01T00:00:00Z", "precision": 9 }
                            }
                        }
                    },
                    {
                        "id": "Q1$pref",
                        "rank": "preferred",
                        "mainsnak": {
                            "snaktype": "value",
                            "property": "P569",
                            "datavalue": {
                                "value": { "time": "+1769-08-15T00:00:00Z", "precision": 11 }
                            }
                        }
                    }
                ]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(identity_year(&parsed, "P569"), Some(1769));
    }

    #[test]
    fn deprecated_p569_is_excluded_from_lifespan_year() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 4,
            "claims": {
                "P569": [{
                    "id": "Q1$dep",
                    "rank": "deprecated",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P569",
                        "datavalue": {
                            "value": { "time": "+1768-01-01T00:00:00Z", "precision": 9 }
                        }
                    }
                }]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].insert.rank, "deprecated");
        assert!(parsed[0].event.is_some());
        assert_eq!(identity_year(&parsed, "P569"), None);
    }

    #[test]
    fn p551_end_time_only_uses_p582_year() {
        let entity = json!({
            "id": "Q517",
            "lastrevid": 7,
            "claims": {
                "P551": [{
                    "id": "Q517$p551-end",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P551",
                        "datavalue": { "value": { "id": "Q90" } }
                    },
                    "qualifiers": {
                        "P582": [{
                            "snaktype": "value",
                            "property": "P582",
                            "datavalue": {
                                "value": {
                                    "time": "+1814-04-06T00:00:00Z",
                                    "precision": 11
                                },
                                "type": "time"
                            }
                        }]
                    }
                }]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].event,
            Some((
                "residence".into(),
                "resided_in".into(),
                Some(1814),
                Some("Q90".into())
            ))
        );
        assert_eq!(
            promoted_statement_lines(&parsed),
            "STATEMENT\tresidence\tresided_in\t1814\tQ90"
        );
    }

    #[test]
    fn bce_mainsnak_year_is_negative() {
        let entity = json!({
            "id": "Q1",
            "lastrevid": 3,
            "claims": {
                "P569": [{
                    "id": "Q1$bce",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P569",
                        "datavalue": {
                            "value": { "time": "-0044-03-15T00:00:00Z", "precision": 11 }
                        }
                    }
                }]
            }
        });
        let parsed = parse_entity_claims(&entity);
        assert_eq!(
            parsed[0].event,
            Some(("birth".into(), "born_in".into(), Some(-44), None))
        );
        assert_eq!(
            promoted_statement_lines(&parsed),
            "STATEMENT\tbirth\tborn_in\t-44\t"
        );
    }
}
