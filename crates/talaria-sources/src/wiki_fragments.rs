// crates/talaria-sources/src/wiki_fragments.rs
use talaria_store::DocumentFragmentInsert;
use talaria_text::fragment_wikitext;
use uuid::Uuid;

/// Map wikitext fragments to store inserts. Parent ids stay `None`;
/// sentence `metadata.parent_section_ordinal` is how the caller wires parents.
pub fn fragment_inserts(snapshot_id: Uuid, wikitext: &str) -> Vec<DocumentFragmentInsert> {
    let mut infoboxes = Vec::new();
    let mut sections = Vec::new();
    let mut sentences = Vec::new();

    for f in fragment_wikitext(wikitext) {
        let insert = DocumentFragmentInsert {
            snapshot_id,
            fragment_kind: f.kind.to_string(),
            parent_fragment_id: None,
            sentence_id: None,
            text: f.text.clone(),
            start_offset: f.start_offset,
            end_offset: f.end_offset,
            clause_index: None,
            ordinal: f.ordinal,
            metadata: fragment_metadata(&f),
        };
        match f.kind {
            "infobox" => infoboxes.push(insert),
            "section" => sections.push(insert),
            _ => sentences.push(insert),
        }
    }

    infoboxes.extend(sections);
    for (i, sentence) in sentences.iter_mut().enumerate() {
        sentence.ordinal = i as i32;
    }
    infoboxes.extend(sentences);
    infoboxes
}

fn fragment_metadata(f: &talaria_text::WikiContentFragment) -> serde_json::Value {
    serde_json::json!({
        "section_path": f.section_path,
        "internal_links": f.internal_links.iter().map(|l| {
            serde_json::json!({
                "surface": l.surface,
                "target_title": l.target_title,
                "qid": l.qid,
            })
        }).collect::<Vec<_>>(),
        "citations": f.citations.iter().map(|c| {
            serde_json::json!({
                "ref_name": c.ref_name,
                "text": c.text,
            })
        }).collect::<Vec<_>>(),
        "parent_section_ordinal": f.parent_section_ordinal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_emits_more_than_one_sentence() {
        let wikitext = "== A ==\nFirst sentence is long enough.\n\n== B ==\nSecond sentence is also long enough.\n";
        let inserts = fragment_inserts(uuid::Uuid::nil(), wikitext);
        assert!(
            inserts
                .iter()
                .filter(|i| i.fragment_kind == "sentence")
                .count()
                >= 2
        );
        assert!(inserts.iter().any(|i| i.fragment_kind == "section"));
    }

    #[test]
    fn sentence_ordinals_are_unique_and_carry_parent_section() {
        let wikitext = "== A ==\nFirst sentence is long enough.\n\n== B ==\nSecond sentence is also long enough.\n";
        let inserts = fragment_inserts(uuid::Uuid::nil(), wikitext);
        let sentences: Vec<_> = inserts
            .iter()
            .filter(|i| i.fragment_kind == "sentence")
            .collect();
        let mut ordinals: Vec<i32> = sentences.iter().map(|i| i.ordinal).collect();
        ordinals.sort();
        let mut unique = ordinals.clone();
        unique.dedup();
        assert_eq!(
            ordinals, unique,
            "sentence ordinals must be unique for the snapshot unique index"
        );
        for s in sentences {
            assert!(s.parent_fragment_id.is_none());
            assert!(s.clause_index.is_none());
            assert!(s.metadata.get("parent_section_ordinal").is_some());
        }
        let first_sentence = inserts
            .iter()
            .position(|i| i.fragment_kind == "sentence")
            .unwrap();
        assert!(inserts[..first_sentence]
            .iter()
            .all(|i| i.fragment_kind == "infobox" || i.fragment_kind == "section"));
    }
}
