// crates/talaria-sources/src/matching.rs
//! Explicable subject↔document matching (no opaque identity merge).

use crate::corpus::{
    EntityDocumentMatch, MatchComponent, NormalizedCorpusDocument, SUBJECT_MATCH_V1,
};
use crate::kinds::ContributionRole;
use crate::plan::ResolvedSubject;

fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'À'..='Å' | 'à'..='å' => 'a',
            'È'..='Ë' | 'è'..='ë' => 'e',
            'Ì'..='Ï' | 'ì'..='ï' => 'i',
            'Ò'..='Ö' | 'ò'..='ö' => 'o',
            'Ù'..='Ü' | 'ù'..='ü' => 'u',
            'Ç' | 'ç' => 'c',
            'Ñ' | 'ñ' => 'n',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn contains_token(hay: &str, needle: &str) -> bool {
    let h = fold(hay);
    let n = fold(needle);
    if n.len() < 3 {
        return false;
    }
    h.contains(&n)
}

/// Bilingual / catalog variants for matching FR theses and EN subject labels.
pub fn subject_match_aliases(label: &str) -> Vec<String> {
    let mut out = vec![label.trim().to_string()];
    let folded = fold(label);

    if folded.contains("christopher") && folded.contains("columbus") {
        out.push("Christophe Colomb".into());
        out.push("Christopher Columbus".into());
        out.push("Colomb, Christophe".into());
        out.push("Columbus, Christopher".into());
        out.push("Colomb".into());
    }
    if folded.contains("christophe") && folded.contains("colomb") {
        out.push("Christopher Columbus".into());
        out.push("Christophe Colomb".into());
        out.push("Colomb, Christophe".into());
        out.push("Colomb".into());
    }
    if folded.contains("napoleon") {
        out.push("Napoléon".into());
        out.push("Napoleon Bonaparte".into());
        out.push("Bonaparte, Napoléon".into());
    }
    if folded.contains("marie") && folded.contains("curie") {
        out.push("Marie Curie".into());
        out.push("Curie, Marie".into());
    }

    let words: Vec<&str> = label.split_whitespace().filter(|w| !w.is_empty()).collect();
    if words.len() >= 2 {
        let first = words[0];
        let last = words[words.len() - 1];
        out.push(format!("{last}, {first}"));
        if last.len() >= 6 {
            out.push(last.to_string());
        }
    }

    out.sort();
    out.dedup();
    out.retain(|s| s.len() >= 3);
    out
}

fn distinctive_surname_in_text(text: &str, aliases: &[String]) -> Option<String> {
    for alias in aliases {
        let last = alias.rsplit(' ').next().unwrap_or("");
        if last.len() >= 5 && contains_token(text, last) {
            return Some(last.to_string());
        }
    }
    None
}

fn text_mentions_alias(text: &str, aliases: &[String]) -> Option<String> {
    for alias in aliases {
        if contains_token(text, alias) {
            return Some(alias.clone());
        }
        let words: Vec<&str> = alias.split_whitespace().collect();
        if words.len() >= 2 {
            let first = words[0];
            let last = words[words.len() - 1];
            if contains_token(text, first) && contains_token(text, last) {
                return Some(format!("{first} … {last}"));
            }
            let inverted = format!("{last}, {first}");
            if contains_token(text, &inverted) {
                return Some(inverted);
            }
        }
    }
    distinctive_surname_in_text(text, aliases).map(|s| format!("surname {s}"))
}

/// Score how a historical subject relates to a bibliographic document.
/// Default relation is `about` (title/abstract/subjects). `by` only when an
/// author name clearly equals the subject label (rare for historical figures).
pub fn match_subject_to_document(
    subject_label: &str,
    doc: &NormalizedCorpusDocument,
) -> Option<EntityDocumentMatch> {
    let aliases = subject_match_aliases(subject_label);
    match_subject_with_aliases(subject_label, &aliases, doc)
}

pub fn match_resolved_subject_to_document(
    subject: &ResolvedSubject,
    doc: &NormalizedCorpusDocument,
) -> Option<EntityDocumentMatch> {
    let mut aliases = subject_match_aliases(&subject.label);
    for (scheme, value) in &subject.known_identifiers {
        if scheme == "wikidata" && !value.is_empty() {
            aliases.push(value.clone());
        }
    }
    aliases.sort();
    aliases.dedup();
    match_subject_with_aliases(&subject.label, &aliases, doc)
}

fn match_subject_with_aliases(
    subject_label: &str,
    aliases: &[String],
    doc: &NormalizedCorpusDocument,
) -> Option<EntityDocumentMatch> {
    let mut components = Vec::new();
    let mut score = 0.0_f32;

    if let Some(hit) = text_mentions_alias(&doc.title, aliases) {
        let weight = if hit.starts_with("surname ") {
            0.40
        } else {
            0.45
        };
        components.push(MatchComponent {
            key: "title_overlap".into(),
            weight,
            detail: format!("title mentions `{hit}`"),
        });
        score += weight;
    }

    if let Some(abs) = &doc.abstract_text {
        if let Some(hit) = text_mentions_alias(abs, aliases) {
            let weight = if hit.starts_with("surname ") || !hit.contains(' ') {
                0.35
            } else {
                0.25
            };
            components.push(MatchComponent {
                key: "abstract_overlap".into(),
                weight,
                detail: format!("abstract mentions `{hit}`"),
            });
            score += weight;
        }
    }

    if let Some(hit) = text_mentions_alias(&doc.snapshot_text, aliases) {
        if !components.iter().any(|c| c.key == "title_overlap" || c.key == "abstract_overlap") {
            components.push(MatchComponent {
                key: "snapshot_overlap".into(),
                weight: 0.20,
                detail: format!("snapshot mentions `{hit}`"),
            });
            score += 0.20;
        }
    }

    let mut rameau_hit = false;
    let mut keyword_hit = false;
    for subj in &doc.subjects {
        if text_mentions_alias(&subj.label, aliases).is_some() {
            if subj.scheme == "rameau" {
                rameau_hit = true;
            } else {
                keyword_hit = true;
            }
        }
    }
    if rameau_hit {
        components.push(MatchComponent {
            key: "rameau_hit".into(),
            weight: 0.30,
            detail: format!("RAMEAU subject mentions `{subject_label}`"),
        });
        score += 0.30;
    } else if keyword_hit {
        components.push(MatchComponent {
            key: "keyword_hit".into(),
            weight: 0.15,
            detail: format!("free keyword mentions `{subject_label}`"),
        });
        score += 0.15;
    }

    let mut relation = "about".to_string();
    let author_hit = doc.contributions.iter().any(|c| {
        c.role == ContributionRole::Author
            && text_mentions_alias(&c.agent_name, aliases).is_some()
    });
    if author_hit {
        components.push(MatchComponent {
            key: "author_name_hit".into(),
            weight: 0.35,
            detail: format!("author name overlaps `{subject_label}`"),
        });
        score += 0.35;
        relation = "by".into();
    }

    if components.is_empty() || score < 0.30 {
        return None;
    }

    let score = score.min(1.0);
    let evidence_summary = components
        .iter()
        .map(|c| c.key.as_str())
        .collect::<Vec<_>>()
        .join(",");

    Some(EntityDocumentMatch {
        relation,
        match_version: SUBJECT_MATCH_V1.into(),
        score,
        components,
        evidence_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{AcademicStatus, AccessLevel, DocumentType, SourceKind};
    use crate::types::TypedTimeLite;

    fn sample_doc(title: &str, abstract_text: Option<&str>) -> NormalizedCorpusDocument {
        NormalizedCorpusDocument {
            source_kind: SourceKind::ThesesFr,
            external_id: "2020ABCD1234".into(),
            canonical_url: None,
            document_type: DocumentType::Thesis,
            title: title.into(),
            language: Some("fr".into()),
            abstract_text: abstract_text.map(str::to_string),
            academic_status: AcademicStatus::DoctoralDefended,
            access_level: AccessLevel::MetadataOnly,
            full_text_available: false,
            rights_uri: None,
            rights_holder: None,
            rights_normalized: AccessLevel::Open,
            publisher_or_institution: None,
            publication_time: TypedTimeLite::Unknown { surface: None },
            identifiers: vec![],
            contributions: vec![],
            subjects: vec![],
            connector_version: "test".into(),
            snapshot_text: abstract_text.unwrap_or(title).into(),
            revision_token: None,
            raw_metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn title_match_about_napoleon() {
        let doc = sample_doc("Napoléon et l'Europe", None);
        let m = match_subject_to_document("Napoleon", &doc).unwrap();
        assert_eq!(m.relation, "about");
        assert!(m.score >= 0.30);
        assert_eq!(m.match_version, SUBJECT_MATCH_V1);
    }

    #[test]
    fn unrelated_title_rejected() {
        let doc = sample_doc("Histoire de la chimie organique", None);
        assert!(match_subject_to_document("Napoleon", &doc).is_none());
    }

    #[test]
    fn columbus_matches_french_thesis_title() {
        let doc = sample_doc(
            "Christophe Colomb et la découverte de l'Amérique : historiographie et débat",
            Some("Cette thèse examine les origines génoises de Colomb."),
        );
        let m = match_subject_to_document("Christopher Columbus", &doc).unwrap();
        assert_eq!(m.relation, "about");
        assert!(m.components.iter().any(|c| c.key == "title_overlap"));
    }

    #[test]
    fn columbus_matches_surname_and_first_in_abstract() {
        let doc = sample_doc(
            "La navigation atlantique au XVe siècle",
            Some("L'itinéraire de Colomb reste disputé par les historiens."),
        );
        let m = match_subject_to_document("Christopher Columbus", &doc).unwrap();
        assert!(m.components.iter().any(|c| c.key == "abstract_overlap"));
    }

    #[test]
    fn aliases_include_bilingual_forms() {
        let aliases = subject_match_aliases("Christopher Columbus");
        assert!(aliases.iter().any(|a| a.contains("Christophe")));
        assert!(aliases.iter().any(|a| a.contains("Colomb")));
    }
}
