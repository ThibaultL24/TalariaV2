// crates/talaria-store/src/visit_catalog.rs
//! Heritage catalog rules. A city coordinate is not a monument.

use serde_json::{json, Value};

const SETTLEMENTS: &[&str] = &[
    "Q515", "Q1549591", "Q3957", "Q532", "Q6256", "Q3624078", "Q82794", "Q35657", "Q70208",
    "Q56061", "Q484170", "Q7930989", "Q1549591",
];

const REGIONS: &[&str] = &["Q82794", "Q35657", "Q3624078", "Q6256", "Q56061"];

const LIVED_SITES: &[&str] = &[
    "Q33506", "Q207694", "Q3947", "Q16560", "Q16970", "Q2977", "Q23413", "Q751876", "Q123705",
    "Q39614", "Q162875", "Q3914", "Q3918",
];

const MEMORY_SITES: &[&str] = &[
    "Q33506", "Q207694", "Q4989906", "Q5003624", "Q179700", "Q162875", "Q39614", "Q123705",
];

const SITE_LABEL_CUES: &[&str] = &[
    "house", "maison", "hôtel", "hotel", "château", "chateau", "musée", "musee", "museum",
    "cathédrale", "cathedrale", "cathedral", "église", "eglise", "tombe", "tomb", "palais",
    "palace", "mémorial", "memorial", "statue", "abbaye", "manoir", "villa", "maison-musée",
];

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct PlaceProfile {
    pub qid: String,
    pub label: String,
    pub instance_of: Vec<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub image_file: Option<String>,
    pub revision_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum ClaimDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PlaceClaim {
    pub pid: String,
    pub target_qid: String,
    pub direction: ClaimDirection,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct MerimeeHit {
    pub ref_id: String,
    pub title: String,
    pub hist: Option<String>,
    pub personalities: Vec<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    #[serde(default)]
    pub protection_kind: Option<String>,
    #[serde(default)]
    pub commune: Option<String>,
    /// Structured `(name, role)` pairs from Personnes_liees_a_l_edifice — not from free text.
    #[serde(default)]
    pub structured_roles: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct CommonsCandidate {
    pub commons_file: String,
    pub depicts_qids: Vec<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub thumb_url: Option<String>,
    pub site_qid: String,
    pub person_qid: String,
    pub is_site_p18: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedMedia {
    pub commons_file: String,
    pub license: String,
    pub attribution_text: String,
    pub thumb_url: Option<String>,
    pub depicts_qids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SiteRelation {
    pub relation: String,
    pub temporal_class: String,
    pub provenance: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSite {
    pub key: String,
    pub label: String,
    pub place_qid: Option<String>,
    pub merimee_ref: Option<String>,
    pub category: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub summary: Option<String>,
    pub relations: Vec<SiteRelation>,
    pub media: Option<AcceptedMedia>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedHit {
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogReport {
    pub candidates: usize,
    pub accepted: Vec<AcceptedSite>,
    pub rejected: Vec<RejectedHit>,
    pub duplicates: usize,
}

pub fn specific_site_label(label: &str) -> bool {
    let folded = fold(label);
    SITE_LABEL_CUES.iter().any(|cue| folded.contains(cue))
}

pub fn catalog_heritage(
    person_qid: &str,
    person_label: &str,
    profiles: &[PlaceProfile],
    claims: &[PlaceClaim],
    merimee: &[MerimeeHit],
    media: &[CommonsCandidate],
) -> CatalogReport {
    let mut accepted: Vec<AcceptedSite> = Vec::new();
    let mut rejected = Vec::new();
    let mut duplicates = 0_usize;
    let candidates = claims.len() + merimee.len();

    for claim in claims {
        let Some(profile) = profiles.iter().find(|profile| profile.qid == claim.target_qid) else {
            rejected.push(RejectedHit {
                key: claim.target_qid.clone(),
                reason: "unresolved_place".into(),
            });
            continue;
        };
        match decide_claim(person_qid, claim, profile) {
            Ok(site) => merge_site(&mut accepted, site, &mut duplicates),
            Err(reason) => rejected.push(RejectedHit {
                key: claim.target_qid.clone(),
                reason,
            }),
        }
    }

    for hit in merimee {
        match decide_merimee(person_label, hit) {
            Ok(site) => merge_site(&mut accepted, site, &mut duplicates),
            Err(reason) => rejected.push(RejectedHit {
                key: format!("merimee:{}", hit.ref_id),
                reason,
            }),
        }
    }

    for item in media {
        let Some(site) = accepted.iter_mut().find(|site| site.place_qid.as_deref() == Some(item.site_qid.as_str())) else {
            rejected.push(RejectedHit {
                key: item.commons_file.clone(),
                reason: "media_without_site".into(),
            });
            continue;
        };
        match decide_media(item) {
            Ok(asset) => {
                if site.media.is_some() {
                    duplicates += 1;
                } else {
                    site.media = Some(asset);
                }
            }
            Err(reason) => rejected.push(RejectedHit {
                key: item.commons_file.clone(),
                reason,
            }),
        }
    }

    CatalogReport {
        candidates,
        accepted,
        rejected,
        duplicates,
    }
}

fn decide_claim(person_qid: &str, claim: &PlaceClaim, profile: &PlaceProfile) -> Result<AcceptedSite, String> {
    if !matches_relation(claim, &profile.instance_of) {
        if is_settlement(&profile.instance_of) || is_region(&profile.instance_of) {
            return Err(if is_region(&profile.instance_of) { "region_grain" } else { "settlement_grain" }.into());
        }
        return Err("not_a_heritage_site".into());
    }
    let (relation, temporal, category) = match (claim.direction, claim.pid.as_str()) {
        (ClaimDirection::Outgoing, "P19") => ("birthplace", "lived", "other"),
        (ClaimDirection::Outgoing, "P20" | "P119") => ("burial_site", "lived", "burial"),
        (ClaimDirection::Outgoing, "P551") => ("former_residence", "lived", "residence"),
        (ClaimDirection::Outgoing, "P937" | "P69") => ("workplace", "lived", "other"),
        (ClaimDirection::Incoming, "P547" | "P138" | "P825") => ("memorial", "memory", "memorial"),
        _ => return Err("unsupported_relation".into()),
    };
    Ok(AcceptedSite {
        key: profile.qid.clone(),
        label: profile.label.clone(),
        place_qid: Some(profile.qid.clone()),
        merimee_ref: None,
        category: category.into(),
        lat: profile.lat,
        lon: profile.lon,
        summary: None,
        relations: vec![SiteRelation {
            relation: relation.into(),
            temporal_class: temporal.into(),
            provenance: json!({
                "source": "wikidata",
                "pid": claim.pid,
                "person_qid": person_qid,
                "place_qid": profile.qid,
                "revision_id": profile.revision_id,
                "instance_of": profile.instance_of,
            }),
        }],
        media: None,
    })
}

fn decide_merimee(person_label: &str, hit: &MerimeeHit) -> Result<AcceptedSite, String> {
    let mentioned = hit.personalities.iter().any(|name| person_named_in(person_label, name));
    if !mentioned {
        return Err("person_not_in_source".into());
    }
    if hit.title.trim().is_empty() || hit.ref_id.trim().is_empty() {
        return Err("missing_site_identity".into());
    }
    let matched_role = hit
        .structured_roles
        .iter()
        .find(|(name, _)| person_named_in(person_label, name))
        .and_then(|(_, role)| role.as_deref());
    let (relation, temporal, category) = relation_from_structured_role(matched_role);
    Ok(AcceptedSite {
        key: format!("merimee:{}", hit.ref_id),
        label: hit.title.clone(),
        place_qid: None,
        merimee_ref: Some(hit.ref_id.clone()),
        category: category.into(),
        lat: hit.lat,
        lon: hit.lon,
        summary: hit.hist.clone(),
        relations: vec![SiteRelation {
            relation: relation.into(),
            temporal_class: temporal.into(),
            provenance: json!({
                "source": "merimee_local",
                "ref": hit.ref_id,
                "personalities": hit.personalities,
                "role": matched_role,
                "protection_kind": hit.protection_kind,
                "commune": hit.commune,
            }),
        }],
        media: None,
    })
}

fn relation_from_structured_role(role: Option<&str>) -> (&'static str, &'static str, &'static str) {
    let folded = role.map(fold).unwrap_or_default();
    if folded.contains("habitant") || folded.contains("resid") {
        ("former_residence", "lived", "residence")
    } else if folded.contains("sepult") || folded.contains("tombe") || folded.contains("inhum") {
        ("burial_site", "lived", "burial")
    } else {
        ("honoree", "memory", "monument")
    }
}

pub fn decide_media(item: &CommonsCandidate) -> Result<AcceptedMedia, String> {
    let depicts_site = item.depicts_qids.iter().any(|qid| qid == &item.site_qid);
    let depicts_person = item.depicts_qids.iter().any(|qid| qid == &item.person_qid);
    if depicts_person && !depicts_site && !item.is_site_p18 {
        return Err("portrait_not_a_place".into());
    }
    if !depicts_site && !item.is_site_p18 {
        return Err("unrelated_media".into());
    }
    let Some(author) = item.author.as_deref().filter(|value| !value.trim().is_empty()) else {
        return Err("missing_attribution".into());
    };
    let Some(license) = item.license.as_deref().filter(|value| !value.trim().is_empty()) else {
        return Err("missing_license".into());
    };
    Ok(AcceptedMedia {
        commons_file: item.commons_file.clone(),
        license: license.to_string(),
        attribution_text: format!("{author} / {license}"),
        thumb_url: item.thumb_url.clone(),
        depicts_qids: item.depicts_qids.clone(),
    })
}

pub fn person_named_in(person_label: &str, text: &str) -> bool {
    let label_words = words(person_label);
    let haystack = words(text);
    if label_words.len() >= 2 && haystack.windows(label_words.len()).any(|window| window == label_words.as_slice()) {
        return true;
    }
    let tokens: Vec<&str> = label_words.iter().filter(|token| token.len() >= 3).map(String::as_str).collect();
    !tokens.is_empty() && tokens.iter().all(|token| haystack.iter().any(|word| word == token))
}

fn words(value: &str) -> Vec<String> {
    fold(value)
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

fn merge_site(accepted: &mut Vec<AcceptedSite>, site: AcceptedSite, duplicates: &mut usize) {
    if let Some(existing) = accepted.iter_mut().find(|item| item.key == site.key) {
        *duplicates += 1;
        for relation in site.relations {
            if existing.relations.iter().any(|item| item.relation == relation.relation) {
                *duplicates += 1;
            } else {
                existing.relations.push(relation);
            }
        }
        if existing.lat.is_none() {
            existing.lat = site.lat;
            existing.lon = site.lon;
        }
        return;
    }
    accepted.push(site);
}

fn matches_relation(claim: &PlaceClaim, instance_of: &[String]) -> bool {
    let lived = instance_of.iter().any(|qid| LIVED_SITES.contains(&qid.as_str()));
    let memory = instance_of.iter().any(|qid| MEMORY_SITES.contains(&qid.as_str()));
    match claim.direction {
        ClaimDirection::Outgoing => lived,
        ClaimDirection::Incoming => memory,
    }
}

fn is_settlement(instance_of: &[String]) -> bool {
    instance_of.iter().any(|qid| SETTLEMENTS.contains(&qid.as_str()))
}

fn is_region(instance_of: &[String]) -> bool {
    instance_of.iter().any(|qid| REGIONS.contains(&qid.as_str()))
}

fn fold(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn city() -> PlaceProfile {
        PlaceProfile {
            qid: "Q90".into(),
            label: "Paris".into(),
            instance_of: vec!["Q515".into()],
            lat: Some(48.86),
            lon: Some(2.35),
            image_file: None,
            revision_id: Some("1".into()),
        }
    }

    fn house() -> PlaceProfile {
        PlaceProfile {
            qid: "Q3129162".into(),
            label: "Hauteville House".into(),
            instance_of: vec!["Q3947".into()],
            lat: Some(49.45),
            lon: Some(-2.53),
            image_file: Some("Hauteville.jpg".into()),
            revision_id: Some("9".into()),
        }
    }

    #[test]
    fn paris_birth_is_not_a_monument() {
        let report = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[city()],
            &[PlaceClaim { pid: "P19".into(), target_qid: "Q90".into(), direction: ClaimDirection::Outgoing }],
            &[],
            &[],
        );
        assert!(report.accepted.is_empty());
        assert_eq!(report.rejected[0].reason, "settlement_grain");
        assert!(!specific_site_label("Paris"));
        assert!(specific_site_label("Hauteville House"));
    }

    #[test]
    fn house_and_memorial_share_one_site_and_keep_sources() {
        let place = PlaceProfile {
            qid: "Q3129162".into(),
            label: "Hauteville House".into(),
            instance_of: vec!["Q3947".into(), "Q5003624".into()],
            lat: Some(49.45),
            lon: Some(-2.53),
            image_file: None,
            revision_id: Some("9".into()),
        };
        let report = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[place],
            &[
                PlaceClaim { pid: "P551".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Outgoing },
                PlaceClaim { pid: "P551".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Outgoing },
                PlaceClaim { pid: "P547".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Incoming },
            ],
            &[],
            &[],
        );
        assert_eq!(report.accepted.len(), 1);
        assert!(report.duplicates >= 1);
        let relations: Vec<_> = report.accepted[0].relations.iter().map(|item| item.relation.as_str()).collect();
        assert!(relations.contains(&"former_residence"));
        assert!(relations.contains(&"memorial"));
        assert!(report.accepted[0].relations.iter().any(|item| item.temporal_class == "memory"));
        assert!(report.accepted[0].relations.iter().all(|item| item.provenance["source"] == "wikidata"));
        assert!(report.accepted[0].relations.iter().all(|item| item.temporal_class != "lived" || item.relation != "memorial"));
    }

    #[test]
    fn two_people_can_reference_the_same_site_key() {
        let claim = PlaceClaim { pid: "P551".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Outgoing };
        let hugo = catalog_heritage("Q535", "Victor Hugo", &[house()], &[claim.clone()], &[], &[]);
        let other = catalog_heritage("Q517", "Napoleon", &[house()], &[claim], &[], &[]);
        assert_eq!(hugo.accepted[0].key, other.accepted[0].key);
    }

    #[test]
    fn merimee_relation_ignores_free_text_history() {
        let from_hist = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[],
            &[],
            &[MerimeeHit {
                ref_id: "PA002".into(),
                title: "Monument".into(),
                hist: Some("Victor Hugo y habita longtemps.".into()),
                personalities: vec!["Hugo Victor".into()],
                lat: None,
                lon: None,
                protection_kind: None,
                commune: None,
                structured_roles: vec![("Hugo Victor".into(), None)],
            }],
            &[],
        );
        assert_eq!(from_hist.accepted[0].relations[0].relation, "honoree");
    }

    #[test]
    fn merimee_requires_the_person_and_invents_no_hours() {
        let missed = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[],
            &[],
            &[MerimeeHit {
                ref_id: "PA001".into(),
                title: "Immeuble parisien".into(),
                hist: Some("Situé à Paris".into()),
                personalities: vec!["Autre personne".into()],
                lat: Some(48.8),
                lon: Some(2.3),
                protection_kind: None,
                commune: None,
                structured_roles: vec![],
            }],
            &[],
        );
        assert_eq!(missed.rejected[0].reason, "person_not_in_source");
        let found = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[],
            &[],
            &[MerimeeHit {
                ref_id: "PA001".into(),
                title: "Maison de Victor Hugo".into(),
                hist: Some("Victor Hugo y habita.".into()),
                personalities: vec!["Hugo Victor (habitant célèbre)".into()],
                lat: Some(48.85),
                lon: Some(2.36),
                protection_kind: Some("arrêté".into()),
                commune: Some("Paris".into()),
                structured_roles: vec![("Hugo Victor".into(), Some("habitant célèbre".into()))],
            }],
            &[],
        );
        assert_eq!(found.accepted.len(), 1);
        assert_eq!(found.accepted[0].relations[0].relation, "former_residence");
        assert!(found.accepted[0].relations[0].provenance["source"] == "merimee_local");
    }

    #[test]
    fn commons_portrait_is_not_a_place_photo() {
        let portrait = CommonsCandidate {
            commons_file: "Hugo portrait.jpg".into(),
            depicts_qids: vec!["Q535".into()],
            author: Some("Nadar".into()),
            license: Some("Public domain".into()),
            thumb_url: Some("https://example.test/p.jpg".into()),
            site_qid: "Q3129162".into(),
            person_qid: "Q535".into(),
            is_site_p18: false,
        };
        assert_eq!(decide_media(&portrait).unwrap_err(), "portrait_not_a_place");
        let photo = CommonsCandidate {
            commons_file: "Hauteville.jpg".into(),
            depicts_qids: vec!["Q3129162".into()],
            author: Some("Example".into()),
            license: Some("CC BY-SA 4.0".into()),
            thumb_url: Some("https://example.test/h.jpg".into()),
            site_qid: "Q3129162".into(),
            person_qid: "Q535".into(),
            is_site_p18: false,
        };
        let report = catalog_heritage(
            "Q535",
            "Victor Hugo",
            &[house()],
            &[PlaceClaim { pid: "P551".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Outgoing }],
            &[],
            &[photo],
        );
        assert_eq!(report.accepted[0].media.as_ref().unwrap().license, "CC BY-SA 4.0");
    }

    #[test]
    fn repeated_catalog_is_stable() {
        let claims = [PlaceClaim { pid: "P551".into(), target_qid: "Q3129162".into(), direction: ClaimDirection::Outgoing }];
        let first = catalog_heritage("Q535", "Victor Hugo", &[house()], &claims, &[], &[]);
        let second = catalog_heritage("Q535", "Victor Hugo", &[house()], &claims, &[], &[]);
        assert_eq!(first.accepted[0].key, second.accepted[0].key);
        assert_eq!(first.accepted.len(), second.accepted.len());
    }
}
