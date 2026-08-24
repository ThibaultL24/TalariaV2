// crates/talaria-api/src/wiki_persist.rs
//! Persist Wikipedia wikitext as section/sentence document_fragments.

use std::collections::HashMap;

use talaria_sources::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use talaria_store::DocumentFragmentInsert;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikipediaSnapshotPayload {
    pub text: String,
    pub plain_extract: String,
    pub source_form: &'static str,
    pub qid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WikiFragmentSet {
    pub first_sentence: Uuid,
    pub sentences: Vec<(Uuid, String)>,
    pub total: usize,
}

pub fn looks_like_wikitext(text: &str) -> bool {
    text.contains("{{") || text.contains("\n==") || text.starts_with("==")
}

/// Skip re-insert when the snapshot already has fragments (`existing_frags == 0`).
pub fn should_insert_wiki_fragments(existing_count: i64) -> bool {
    existing_count == 0
}

/// Quality ingest / Lot E: persist wiki fragments unless `source_form` is plaintext.
/// Dump files still use [`looks_like_wikitext`].
pub fn wikipedia_quality_uses_wiki_fragments(source_form: Option<&str>) -> bool {
    source_form != Some("plain")
}

pub fn pageprops_qid(page: &serde_json::Value) -> Option<String> {
    page.pointer("/pageprops/wikibase_item")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn wikipedia_snapshot_payload(
    extract: &str,
    wikitext: Option<&str>,
    qid: Option<&str>,
) -> WikipediaSnapshotPayload {
    let qid = qid
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    match wikitext.filter(|s| !s.trim().is_empty()) {
        Some(wt) => WikipediaSnapshotPayload {
            text: wt.to_string(),
            plain_extract: extract.to_string(),
            source_form: "wiki",
            qid,
        },
        None => WikipediaSnapshotPayload {
            text: extract.to_string(),
            plain_extract: extract.to_string(),
            source_form: "plain",
            qid,
        },
    }
}

/// Wire sentence `parent_fragment_id` from `metadata.parent_section_ordinal`
/// using **section ordinal → uuid**. Do not use remumbered sentence.ordinal.
pub fn wire_parent_ids(
    inserts: &mut [DocumentFragmentInsert],
    assigned_ids: &[Uuid],
) -> Option<Uuid> {
    let mut section_ids = HashMap::<i32, Uuid>::new();
    let mut first_sentence = None;
    for (i, ins) in inserts.iter_mut().enumerate() {
        let id = assigned_ids.get(i).copied()?;
        wire_one_parent(ins, &section_ids);
        note_section_id(ins, id, &mut section_ids);
        if ins.fragment_kind == "sentence" && first_sentence.is_none() {
            first_sentence = Some(id);
        }
    }
    first_sentence
}

fn wire_one_parent(ins: &mut DocumentFragmentInsert, section_ids: &HashMap<i32, Uuid>) {
    if ins.fragment_kind == "sentence" {
        if let Some(ord) = ins
            .metadata
            .get("parent_section_ordinal")
            .and_then(|v| v.as_i64())
        {
            ins.parent_fragment_id = section_ids.get(&(ord as i32)).copied();
        }
    }
}

fn note_section_id(
    ins: &DocumentFragmentInsert,
    id: Uuid,
    section_ids: &mut HashMap<i32, Uuid>,
) {
    if ins.fragment_kind == "section" {
        section_ids.insert(ins.ordinal, id);
    }
}

pub fn fragment_id_for_clause(
    clause_text: &str,
    sentences: &[(Uuid, String)],
    fallback: Uuid,
) -> Uuid {
    let needle = clause_text.trim();
    if needle.is_empty() {
        return fallback;
    }
    sentences
        .iter()
        .find(|(_, text)| text.contains(needle) || needle.contains(text.trim()))
        .map(|(id, _)| *id)
        .unwrap_or(fallback)
}

fn is_page_level_extractor(id: &str) -> bool {
    matches!(id, "infobox" | "structured_statement" | "military_campaign")
}

/// Infobox/structured once on full wikitext; military once on plaintext;
/// remaining prose extractors per sentence fragment.
pub fn run_wiki_extractors(
    extractors: &[&dyn CandidateExtractor],
    page_title: Option<String>,
    subject_label: Option<String>,
    document_type: String,
    subject_death_year: Option<i32>,
    wikitext: Option<String>,
    known_places: Vec<String>,
    full_plain: &str,
    sentences: &[(Uuid, String)],
    first_sentence: Uuid,
) -> Vec<(Uuid, RawCandidate)> {
    let mut out = Vec::new();
    let make_input = |text: String| ExtractorInput {
        text,
        page_title: page_title.clone(),
        subject_label: subject_label.clone(),
        document_type: document_type.clone(),
        subject_death_year,
        wikitext: wikitext.clone(),
        known_places: known_places.clone(),
    };
    for ex in extractors {
        let id = ex.extractor_id();
        if is_page_level_extractor(id) {
            let text = if id == "infobox" {
                wikitext.clone().unwrap_or_else(|| full_plain.to_string())
            } else {
                full_plain.to_string()
            };
            for raw in ex.extract(&make_input(text)) {
                let fid = fragment_id_for_clause(&raw.clause_text, sentences, first_sentence);
                out.push((fid, raw));
            }
        } else if sentences.is_empty() {
            for raw in ex.extract(&make_input(full_plain.to_string())) {
                out.push((first_sentence, raw));
            }
        } else {
            for (sid, sent) in sentences {
                for raw in ex.extract(&make_input(sent.clone())) {
                    out.push((*sid, raw));
                }
            }
        }
    }
    out
}

pub async fn insert_wiki_fragments(
    pool: &sqlx::PgPool,
    snapshot_id: Uuid,
    wikitext: &str,
) -> anyhow::Result<Uuid> {
    Ok(persist_wiki_fragments(pool, snapshot_id, wikitext)
        .await?
        .first_sentence)
}

/// Stamp `needs_review` when Wikisource proofread quality is problematic.
pub fn merge_proofread_metadata(
    inserts: &mut [DocumentFragmentInsert],
    wikitext: &str,
) {
    if !talaria_sources::connectors::proofread_needs_review(wikitext) {
        return;
    }
    for ins in inserts.iter_mut() {
        ins.metadata["needs_review"] = serde_json::Value::Bool(true);
    }
}

pub async fn persist_wiki_fragments(
    pool: &sqlx::PgPool,
    snapshot_id: Uuid,
    wikitext: &str,
) -> anyhow::Result<WikiFragmentSet> {
    let existing: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM document_fragments WHERE snapshot_id = $1
        "#,
    )
    .bind(snapshot_id)
    .fetch_one(pool)
    .await?;
    if !should_insert_wiki_fragments(existing) {
        return existing_wiki_fragment_set(pool, snapshot_id).await;
    }
    let mut inserts = talaria_sources::fragment_inserts(snapshot_id, wikitext);
    merge_proofread_metadata(&mut inserts, wikitext);
    let mut section_ids = HashMap::<i32, Uuid>::new();
    let mut first_sentence = None;
    let mut sentences = Vec::new();
    let total = inserts.len();
    for mut ins in inserts {
        if ins.fragment_kind == "sentence" {
            if let Some(ord) = ins
                .metadata
                .get("parent_section_ordinal")
                .and_then(|v| v.as_i64())
            {
                ins.parent_fragment_id = section_ids.get(&(ord as i32)).copied();
            }
        }
        let id = talaria_store::insert_document_fragment(pool, &ins).await?;
        if ins.fragment_kind == "section" {
            section_ids.insert(ins.ordinal, id);
        }
        if ins.fragment_kind == "sentence" {
            if first_sentence.is_none() {
                first_sentence = Some(id);
            }
            sentences.push((id, ins.text));
        }
    }
    let first_sentence =
        first_sentence.ok_or_else(|| anyhow::anyhow!("no sentence fragments"))?;
    Ok(WikiFragmentSet {
        first_sentence,
        sentences,
        total,
    })
}

async fn existing_wiki_fragment_set(
    pool: &sqlx::PgPool,
    snapshot_id: Uuid,
) -> anyhow::Result<WikiFragmentSet> {
    let sentences: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, text FROM document_fragments
        WHERE snapshot_id = $1 AND fragment_kind = 'sentence'
        ORDER BY ordinal ASC
        "#,
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM document_fragments WHERE snapshot_id = $1
        "#,
    )
    .bind(snapshot_id)
    .fetch_one(pool)
    .await?;
    let first_sentence = sentences
        .first()
        .map(|(id, _)| *id)
        .ok_or_else(|| anyhow::anyhow!("no sentence fragments"))?;
    Ok(WikiFragmentSet {
        first_sentence,
        sentences,
        total: total as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    const TWO_SECTIONS: &str =
        "== A ==\nFirst sentence is long enough.\n\n== B ==\nSecond sentence is also long enough.\n";

    fn section_path_title(ins: &talaria_store::DocumentFragmentInsert) -> Option<&str> {
        ins.metadata
            .get("section_path")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
    }

    #[test]
    fn sentences_parent_via_section_ordinal_not_remumbered_sentence() {
        let mut inserts = talaria_sources::fragment_inserts(Uuid::nil(), TWO_SECTIONS);
        let ids: Vec<Uuid> = (0..inserts.len()).map(|_| Uuid::new_v4()).collect();
        let first = wire_parent_ids(&mut inserts, &ids).expect("a sentence fragment");

        let a_idx = inserts
            .iter()
            .position(|i| i.fragment_kind == "section" && section_path_title(i) == Some("A"))
            .expect("section A");
        let b_idx = inserts
            .iter()
            .position(|i| i.fragment_kind == "section" && section_path_title(i) == Some("B"))
            .expect("section B");
        let sent_a = inserts
            .iter()
            .find(|i| i.fragment_kind == "sentence" && section_path_title(i) == Some("A"))
            .expect("sentence in A");
        let sent_b = inserts
            .iter()
            .find(|i| i.fragment_kind == "sentence" && section_path_title(i) == Some("B"))
            .expect("sentence in B");

        assert_eq!(sent_a.parent_fragment_id, Some(ids[a_idx]));
        assert_eq!(sent_b.parent_fragment_id, Some(ids[b_idx]));
        assert_ne!(sent_a.parent_fragment_id, sent_b.parent_fragment_id);
        // Remumbered sentence ordinals are 0,1 — must not be used as parent keys.
        assert_eq!(sent_a.ordinal, 0);
        assert_eq!(sent_b.ordinal, 1);
        assert_ne!(inserts[a_idx].ordinal, sent_a.ordinal);
        let first_sent_idx = inserts
            .iter()
            .position(|i| i.fragment_kind == "sentence")
            .unwrap();
        assert_eq!(first, ids[first_sent_idx]);
    }

    #[test]
    fn snapshot_payload_prefers_wikitext_and_keeps_plain_extract() {
        let wiki = wikipedia_snapshot_payload("plain bio", Some("== Life ==\nBorn."), Some("Q517"));
        assert_eq!(wiki.text, "== Life ==\nBorn.");
        assert_eq!(wiki.plain_extract, "plain bio");
        assert_eq!(wiki.source_form, "wiki");
        assert_eq!(wiki.qid.as_deref(), Some("Q517"));

        let plain = wikipedia_snapshot_payload("plain bio", None, Some("Q517"));
        assert_eq!(plain.text, "plain bio");
        assert_eq!(plain.source_form, "plain");
        assert_eq!(plain.plain_extract, "plain bio");
    }

    #[test]
    fn pageprops_qid_reads_wikibase_item() {
        let page = serde_json::json!({"pageprops": {"wikibase_item": "Q517"}});
        assert_eq!(pageprops_qid(&page).as_deref(), Some("Q517"));
        assert_eq!(pageprops_qid(&serde_json::json!({})), None);
    }

    #[test]
    fn clause_attaches_to_containing_sentence() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let fallback = Uuid::new_v4();
        let sentences = vec![
            (a, "He was born in Ajaccio in 1769.".into()),
            (b, "He died on Saint Helena in 1821.".into()),
        ];
        assert_eq!(
            fragment_id_for_clause("born in Ajaccio", &sentences, fallback),
            a
        );
        assert_eq!(
            fragment_id_for_clause("died on Saint Helena", &sentences, fallback),
            b
        );
        assert_eq!(
            fragment_id_for_clause("no match", &sentences, fallback),
            fallback
        );
    }

    #[test]
    fn skip_insert_when_snapshot_already_has_fragments() {
        assert!(should_insert_wiki_fragments(0));
        assert!(!should_insert_wiki_fragments(1));
        assert!(!should_insert_wiki_fragments(12));
    }

    #[test]
    fn problematic_proofread_marks_fragment_needs_review() {
        let wikitext = "{{PR|problematic}}\nFirst sentence is long enough.\n";
        let mut inserts = talaria_sources::fragment_inserts(Uuid::nil(), wikitext);
        merge_proofread_metadata(&mut inserts, wikitext);
        let sentences: Vec<_> = inserts
            .iter()
            .filter(|i| i.fragment_kind == "sentence")
            .collect();
        assert!(!sentences.is_empty());
        assert!(sentences.iter().all(|i| {
            i.metadata.get("needs_review") == Some(&serde_json::Value::Bool(true))
        }));
    }

    #[test]
    fn quality_ingest_wikipedia_without_wikitext_heuristic() {
        let lead = "Napoleon Bonaparte was born in Ajaccio in 1769 and later crowned emperor.";
        assert!(
            !looks_like_wikitext(lead),
            "looks_like_wikitext stays dump-only"
        );
        assert!(wikipedia_quality_uses_wiki_fragments(Some("wiki")));
        assert!(wikipedia_quality_uses_wiki_fragments(None));
        assert!(!wikipedia_quality_uses_wiki_fragments(Some("plain")));
        let inserts = talaria_sources::fragment_inserts(Uuid::nil(), lead);
        assert!(
            inserts.iter().any(|i| i.fragment_kind == "sentence"),
            "lead-only wikitext without {{{{ or == must still fragment"
        );
    }
}
