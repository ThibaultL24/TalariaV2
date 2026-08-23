// crates/talaria-sources/src/extractors/keywords.rs
//! Dump-style keyword mine: any biography page, no demo subject list.

use talaria_judge::{mine_sentence_with_carry, MineCarry};

use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::seeds::lifespan_year_window;

pub struct KeywordMineExtractor;

impl CandidateExtractor for KeywordMineExtractor {
    fn extractor_id(&self) -> &str {
        "dump_keywords"
    }

    fn version(&self) -> &str {
        "dump_keywords:generic_v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let person = input.effective_subject();
        let page = input.page_title.as_deref().unwrap_or(person.as_str());
        let (lo, hi) = lifespan_year_window(None, input.subject_death_year);
        let _ = (lo, hi);
        let mut carry = MineCarry::default();
        let mut out = Vec::new();
        let mut offset = 0i32;
        for (i, sentence) in crate::extractors::split_prose_units(&input.text)
            .into_iter()
            .enumerate()
        {
            let mined = mine_sentence_with_carry(&sentence, page, &carry);
            for hit in mined {
                let (event_type, predicate) = verb_to_event(&hit.verb);
                let end = offset + sentence.len() as i32;
                out.push(RawCandidate {
                    event_type: event_type.into(),
                    predicate: predicate.into(),
                    subject_surface: person.clone(),
                    time_surface: Some(hit.time),
                    place_surface: Some(hit.place),
                    object_surface: None,
                    participant_surfaces: vec![],
                    clause_text: sentence.clone(),
                    clause_index: i as i32,
                    start_offset: offset,
                    end_offset: end,
                    cross_clause_join: false,
                    extractor_id: hit.extractor.into(),
                    is_posthumous: false,
                    lat: None,
                    lon: None,
                });
            }
            carry.absorb(&sentence, page);
            offset += sentence.len() as i32 + 1;
        }
        out
    }
}

fn verb_to_event(verb: &str) -> (&'static str, &'static str) {
    match verb {
        "born" => ("birth", "born_in"),
        "died" => ("death", "died_in"),
        "married" => ("marriage", "married"),
        "studied" => ("education", "studied_at"),
        "fought" => ("battle", "fought_at"),
        "signed" => ("diplomatic", "signed"),
        "crowned" => ("office", "held_office"),
        "exiled" => ("exile", "exiled_to"),
        "lived" | "moved" => ("residence", "resided_in"),
        "visited" => ("travel", "visited"),
        "met" => ("meeting", "met"),
        "published" => ("publication", "published"),
        "anecdoted" => ("anecdote", "anecdoted"),
        other if other == "sailed" => ("travel", "sailed"),
        _ => ("historical_fact", "occurred_at"),
    }
}
