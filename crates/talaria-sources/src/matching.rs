// crates/talaria-sources/src/matching.rs
//! Explicable subject↔document matching (no opaque identity merge).

use crate::corpus::{
    EntityDocumentMatch, MatchComponent, NormalizedCorpusDocument, SUBJECT_MATCH_V1,
};
use crate::kinds::ContributionRole;

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

/// Score how a historical subject relates to a bibliographic document.
/// Default relation is `about` (title/abstract/subjects). `by` only when an
/// author name clearly equals the subject label (rare for historical figures).
pub fn match_subject_to_document(
    subject_label: &str,
    doc: &NormalizedCorpusDocument,
) -> Option<EntityDocumentMatch> {
    let mut components = Vec::new();
    let mut score = 0.0_f32;

    if contains_token(&doc.title, subject_label) {
        components.push(MatchComponent {
            key: "title_overlap".into(),
            weight: 0.45,
            detail: format!("title contains `{subject_label}`"),
        });
        score += 0.45;
    }

    if let Some(abs) = &doc.abstract_text {
        if contains_token(abs, subject_label) {
            components.push(MatchComponent {
                key: "abstract_overlap".into(),
                weight: 0.25,
                detail: format!("abstract contains `{subject_label}`"),
            });
            score += 0.25;
        }
    }

    let mut rameau_hit = false;
    let mut keyword_hit = false;
    for subj in &doc.subjects {
        if contains_token(&subj.label, subject_label) {
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
        c.role == ContributionRole::Author && contains_token(&c.agent_name, subject_label)
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
            snapshot_text: title.into(),
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
}
