// crates/talaria-sources/src/snac/align.rs
//! In-memory SNAC alignment. Contradictory identifiers stop the decision
//! before a weaker match can become `aligned`.

use uuid::Uuid;

use super::ark::SameAs;

#[derive(Debug, Clone)]
pub struct SubjectAuthorities {
    pub entity_id: Uuid,
    pub qid: Option<String>,
    pub label: String,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub viaf: Option<String>,
    pub isni: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CandidateView {
    pub ark: String,
    pub same_as: SameAs,
    pub label: String,
    pub exist_from_year: Option<i32>,
    pub exist_to_year: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Aligned,
    Ambiguous,
    Conflict,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDecision {
    pub ark: String,
    pub status: LinkStatus,
    pub method: &'static str,
}

pub fn decide_candidates(
    subject: &SubjectAuthorities,
    candidates: &[CandidateView],
    already_aligned_ark: Option<&str>,
) -> Vec<LinkDecision> {
    let mut decisions: Vec<LinkDecision> = candidates
        .iter()
        .map(|candidate| decide_one(subject, candidate))
        .collect();

    let certain: Vec<usize> = decisions
        .iter()
        .enumerate()
        .filter(|(_, decision)| decision.status == LinkStatus::Aligned)
        .map(|(index, _)| index)
        .collect();
    if certain.len() > 1 {
        for index in certain {
            decisions[index].status = LinkStatus::Conflict;
            decisions[index].method = "multiple_certain_matches";
        }
    }

    if let Some(taken) = already_aligned_ark.and_then(super::ark::canonical_ark) {
        for decision in &mut decisions {
            if decision.status == LinkStatus::Aligned && decision.ark == taken {
                decision.status = LinkStatus::Conflict;
                decision.method = "constellation_claimed";
            }
        }
    }
    decisions
}

fn decide_one(subject: &SubjectAuthorities, candidate: &CandidateView) -> LinkDecision {
    let ark = super::ark::canonical_ark(&candidate.ark).unwrap_or_else(|| candidate.ark.clone());
    if candidate.same_as.internal_conflict {
        return decision(ark, LinkStatus::Conflict, "internal_identifier_conflict");
    }

    let qids = candidate.same_as.values.get("wikidata");
    if let Some(qids) = qids.filter(|set| !set.is_empty()) {
        let matches_subject = subject
            .qid
            .as_deref()
            .is_some_and(|qid| qids.len() == 1 && qids.contains(qid));
        if !matches_subject {
            return decision(ark, LinkStatus::Conflict, "conflicting_qid");
        }
        if authorities_disagree(subject, &candidate.same_as) {
            return decision(ark, LinkStatus::Conflict, "conflicting_authority");
        }
        return decision(ark, LinkStatus::Aligned, "qid_same_as");
    }

    if authorities_disagree(subject, &candidate.same_as) {
        return decision(ark, LinkStatus::Conflict, "conflicting_authority");
    }

    let viaf_match = identifier_matches(subject.viaf.as_deref(), &candidate.same_as, "viaf");
    let isni_match = identifier_matches(subject.isni.as_deref(), &candidate.same_as, "isni");
    if viaf_match && isni_match {
        return decision(ark, LinkStatus::Aligned, "viaf_and_isni");
    }
    if viaf_match {
        return decision(ark, LinkStatus::Aligned, "viaf_same_as");
    }
    if isni_match {
        return decision(ark, LinkStatus::Aligned, "isni_same_as");
    }
    if years_cover(subject, candidate) && names_compatible(&subject.label, &candidate.label) {
        return decision(ark, LinkStatus::Ambiguous, "label_and_years");
    }
    decision(ark, LinkStatus::Ambiguous, "label_only")
}

fn decision(ark: String, status: LinkStatus, method: &'static str) -> LinkDecision {
    LinkDecision { ark, status, method }
}

fn authorities_disagree(subject: &SubjectAuthorities, same_as: &SameAs) -> bool {
    sets_disagree(subject.viaf.as_deref(), same_as.values.get("viaf"))
        || sets_disagree(subject.isni.as_deref(), same_as.values.get("isni"))
}

fn sets_disagree(subject_value: Option<&str>, snac_values: Option<&std::collections::BTreeSet<String>>) -> bool {
    let Some(subject_value) = subject_value.filter(|value| !value.is_empty()) else {
        return false;
    };
    let Some(snac_values) = snac_values.filter(|values| !values.is_empty()) else {
        return false;
    };
    !snac_values.contains(subject_value)
}

fn identifier_matches(subject_value: Option<&str>, same_as: &SameAs, scheme: &str) -> bool {
    let Some(subject_value) = subject_value.filter(|value| !value.is_empty()) else {
        return false;
    };
    same_as
        .values
        .get(scheme)
        .is_some_and(|values| values.contains(subject_value))
}

fn years_cover(subject: &SubjectAuthorities, candidate: &CandidateView) -> bool {
    let (Some(from_year), Some(to_year)) = (candidate.exist_from_year, candidate.exist_to_year) else {
        return false;
    };
    if subject.birth_year.is_none() && subject.death_year.is_none() {
        return false;
    }
    if let Some(birth_year) = subject.birth_year {
        if birth_year < from_year || birth_year > to_year {
            return false;
        }
    }
    if let Some(death_year) = subject.death_year {
        if death_year < from_year || death_year > to_year {
            return false;
        }
    }
    true
}

fn names_compatible(subject_label: &str, candidate_label: &str) -> bool {
    let subject = subject_label.to_lowercase();
    let candidate = candidate_label.to_lowercase();
    if subject.is_empty() || candidate.is_empty() {
        return false;
    }
    subject.contains(candidate.trim())
        || candidate.contains(subject.trim())
        || subject.split_whitespace().any(|word| {
            word.len() >= 4 && candidate.split_whitespace().any(|other| other == word)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        decide_candidates, CandidateView, LinkStatus, SubjectAuthorities,
    };
    use crate::snac::ark::parse_same_as;
    use uuid::Uuid;

    fn subject_q517_viaf() -> SubjectAuthorities {
        SubjectAuthorities {
            entity_id: Uuid::nil(),
            qid: Some("Q517".into()),
            label: "Napoleon Bonaparte".into(),
            birth_year: Some(1769),
            death_year: Some(1821),
            viaf: Some("106964661".into()),
            isni: None,
        }
    }

    fn candidate_qid(ark: &str, qid: &str, viaf: &str) -> CandidateView {
        let mut uris = vec![format!("http://www.wikidata.org/entity/{qid}")];
        if !viaf.is_empty() {
            uris.push(format!("https://viaf.org/viaf/{viaf}"));
        }
        let refs: Vec<&str> = uris.iter().map(String::as_str).collect();
        CandidateView {
            ark: ark.into(),
            same_as: parse_same_as(&refs),
            label: "Napoleon".into(),
            exist_from_year: Some(1769),
            exist_to_year: Some(1821),
        }
    }

    #[test]
    fn viaf_match_loses_to_a_different_qid() {
        let decisions = decide_candidates(
            &subject_q517_viaf(),
            &[candidate_qid("ark:/99166/w6other", "Q1", "106964661")],
            None,
        );
        assert_eq!(decisions[0].method, "conflicting_qid");
        assert_eq!(decisions[0].status, LinkStatus::Conflict);
    }

    #[test]
    fn matching_qid_and_different_isni_conflicts() {
        let subject = SubjectAuthorities {
            isni: Some("0000000121264454".into()),
            ..subject_q517_viaf()
        };
        let mut candidate = candidate_qid("ark:/99166/w6curie", "Q517", "");
        let uris = [
            "http://www.wikidata.org/entity/Q517",
            "https://isni.org/isni/0000000000000000",
        ];
        candidate.same_as = parse_same_as(&uris);
        let decisions = decide_candidates(&subject, &[candidate], None);
        assert_eq!(decisions[0].method, "conflicting_authority");
    }

    #[test]
    fn viaf_alone_aligns_when_isni_absent() {
        let subject = SubjectAuthorities {
            qid: Some("Q7186".into()),
            viaf: Some("106964661".into()),
            isni: None,
            ..subject_q517_viaf()
        };
        let candidate = CandidateView {
            ark: "ark:/99166/w6viaf".into(),
            same_as: parse_same_as(&["https://viaf.org/viaf/106964661"]),
            label: "Marie Curie".into(),
            exist_from_year: Some(1867),
            exist_to_year: Some(1934),
        };
        let decisions = decide_candidates(&subject, &[candidate], None);
        assert_eq!(decisions[0].method, "viaf_same_as");
        assert_eq!(decisions[0].status, LinkStatus::Aligned);
    }

    #[test]
    fn two_certain_candidates_align_nobody() {
        let first = candidate_qid("ark:/99166/w6one", "Q517", "");
        let second = candidate_qid("ark:/99166/w6two", "Q517", "");
        let decisions = decide_candidates(&subject_q517_viaf(), &[first, second], None);
        assert_eq!(decisions.len(), 2);
        assert!(decisions
            .iter()
            .all(|decision| decision.method == "multiple_certain_matches"));
        assert!(decisions
            .iter()
            .all(|decision| decision.status == LinkStatus::Conflict));
    }

    #[test]
    fn claimed_constellation_is_not_stolen() {
        let decisions = decide_candidates(
            &subject_q517_viaf(),
            &[candidate_qid("ark:/99166/w6taken", "Q517", "")],
            Some("https://n2t.net/ark:/99166/w6taken"),
        );
        assert_eq!(decisions[0].method, "constellation_claimed");
        assert_eq!(decisions[0].status, LinkStatus::Conflict);
    }

    #[test]
    fn years_outside_the_life_stay_label_only() {
        let subject = SubjectAuthorities {
            qid: None,
            viaf: None,
            isni: None,
            birth_year: Some(1802),
            death_year: Some(1885),
            label: "Victor Hugo".into(),
            ..subject_q517_viaf()
        };
        let candidate = CandidateView {
            ark: "ark:/99166/w6homonym".into(),
            same_as: parse_same_as(&[]),
            label: "Victor Hugo Paltsits".into(),
            exist_from_year: Some(1867),
            exist_to_year: Some(1952),
        };
        let decisions = decide_candidates(&subject, &[candidate], None);
        assert_eq!(decisions[0].method, "label_only");
        assert_eq!(decisions[0].status, LinkStatus::Ambiguous);
    }
}
