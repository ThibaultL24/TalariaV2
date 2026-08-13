// crates/talaria-intuition/src/plan.rs
//! Map Talaria conflict / soft-claim rows to debate bundles.

use crate::canon::{
    atom_record, build_debate_bundle, situated_context_triples, CanonError, DebateBundle,
};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ConflictGroup {
    pub subject_label: String,
    pub occurrence_stem: String,
    pub event_type: String,
    pub time_key: String,
    pub places: Vec<String>,
    pub event_id: Option<String>,
    pub event_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SoftClaimInput {
    pub subject_label: String,
    pub claim_id: String,
    pub claim_kind: String,
    pub text: String,
    pub event_id: Option<String>,
    pub event_title: Option<String>,
    pub place_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedDebate {
    pub kind: String,
    pub debate_id: String,
    pub question_label: String,
    pub proposition_label: String,
    pub bundle: DebateBundle,
}

fn merge_situated(bundle: &mut DebateBundle, event_id: Option<&str>, event_title: Option<&str>, place: Option<&str>) {
    let Some(eid) = event_id.filter(|s| !s.is_empty()) else {
        return;
    };
    let prop = bundle.vote_target.proposition_atom_id.clone();
    let Ok((atoms, triples)) = situated_context_triples(&prop, eid, event_title, place) else {
        return;
    };
    for a in atoms {
        if !bundle.atoms.iter().any(|x| x.id == a.id) {
            bundle.atoms.push(a);
        }
    }
    let roles: Vec<String> = triples.iter().map(|t| t.role.clone()).collect();
    bundle.triples.extend(triples);
    bundle.situated_context = Some(crate::canon::SituatedContext {
        canonical_event_id: eid.to_string(),
        triple_roles: roles,
    });
}

pub fn debate_from_place_conflict(
    group: &ConflictGroup,
    place: &str,
) -> Result<PlannedDebate, CanonError> {
    let q_frag = format!(
        "where-{}-{}-{}",
        group.subject_label, group.event_type, group.time_key
    );
    let p_frag = format!("at-{place}");
    let debate_id = format!(
        "talaria:debate:{}:{}",
        crate::normalize_slug_fragment(&q_frag),
        crate::normalize_slug_fragment(&p_frag)
    );
    let q_title = format!(
        "Where was {} during {} ({})?",
        group.subject_label, group.event_type, group.time_key
    );
    let p_title = format!("{place}");
    let mut bundle = build_debate_bundle(&debate_id, &q_frag, &q_title, &p_frag, &p_title)?;
    merge_situated(
        &mut bundle,
        group.event_id.as_deref(),
        group.event_title.as_deref(),
        Some(place),
    );
    // Keep subject person atom for graph browsers (not on-chain required).
    if let Ok(person) = atom_record("person", &group.subject_label, Some(&group.subject_label)) {
        if !bundle.atoms.iter().any(|a| a.id == person.id) {
            bundle.atoms.push(person);
        }
    }
    Ok(PlannedDebate {
        kind: "place_conflict".into(),
        debate_id,
        question_label: q_title,
        proposition_label: p_title,
        bundle,
    })
}

pub fn debate_from_soft_claim(claim: &SoftClaimInput) -> Result<PlannedDebate, CanonError> {
    let q_frag = format!("about-{}", claim.subject_label);
    let p_frag = format!("claim-{}", claim.claim_id);
    let debate_id = format!(
        "talaria:debate:{}:{}",
        crate::normalize_slug_fragment(&q_frag),
        crate::normalize_slug_fragment(&p_frag)
    );
    let q_title = format!("What is claimed about {}?", claim.subject_label);
    let p_title = claim.text.clone();
    let mut bundle = build_debate_bundle(&debate_id, &q_frag, &q_title, &p_frag, &p_title)?;
    merge_situated(
        &mut bundle,
        claim.event_id.as_deref(),
        claim.event_title.as_deref(),
        claim.place_label.as_deref(),
    );
    Ok(PlannedDebate {
        kind: claim.claim_kind.clone(),
        debate_id,
        question_label: q_title,
        proposition_label: p_title,
        bundle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paris_and_fontainebleau_share_question_differ_on_proposition() {
        let group = ConflictGroup {
            subject_label: "Napoleon".into(),
            occurrence_stem: "stem".into(),
            event_type: "residence".into(),
            time_key: "1814-05-18".into(),
            places: vec!["Paris".into(), "Fontainebleau".into()],
            event_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
            event_title: Some("Residence 1814".into()),
        };
        let a = debate_from_place_conflict(&group, "Paris").unwrap();
        let b = debate_from_place_conflict(&group, "Fontainebleau").unwrap();
        assert_eq!(
            a.bundle.vote_target.question_atom_id,
            b.bundle.vote_target.question_atom_id
        );
        assert_ne!(
            a.bundle.vote_target.proposition_atom_id,
            b.bundle.vote_target.proposition_atom_id
        );
        assert!(a
            .bundle
            .triples
            .iter()
            .any(|t| t.role == "proposition_about_event"));
        assert!(!a.bundle.triples.iter().any(|t| t.role == "born_on"));
    }

    #[test]
    fn soft_claim_is_opinion_not_map_fact() {
        let c = SoftClaimInput {
            subject_label: "Napoleon".into(),
            claim_id: "11111111-1111-1111-1111-111111111111".into(),
            claim_kind: "theory".into(),
            text: "Poisoned on Saint Helena".into(),
            event_id: None,
            event_title: None,
            place_label: None,
        };
        let d = debate_from_soft_claim(&c).unwrap();
        assert_eq!(d.kind, "theory");
        assert!(d
            .bundle
            .triples
            .iter()
            .any(|t| t.role == "question_has_proposition"));
        assert!(!d.bundle.triples.iter().any(|t| t.predicate.contains("born")));
    }
}
