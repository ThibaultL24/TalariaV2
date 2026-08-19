// crates/talaria-sources/src/extractors/dense.rs
use talaria_quality::{ClauseAnalyzeInput, ClauseAnalyzer, DeterministicClauseAnalyzer};

use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct DenseClauseExtractor;

impl CandidateExtractor for DenseClauseExtractor {
    fn extractor_id(&self) -> &str {
        "dense_clause"
    }

    fn version(&self) -> &str {
        "dense_clause:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        // Skip pure STATEMENT / chronology-only docs to avoid double noise.
        if input.document_type == "structured_statement" {
            return vec![];
        }
        if input.document_type == "chronology_list" {
            return vec![];
        }
        let analyzer = DeterministicClauseAnalyzer;
        let mut offset = 0i32;
        let mut out = Vec::new();
        for sentence in split_keep(input.text.as_str()) {
            let xs = analyzer.analyze_sentence(&ClauseAnalyzeInput {
                text: sentence.clone(),
                page_title: input.page_title.clone(),
                start_offset: offset,
            });
            for x in xs {
                out.push(RawCandidate {
                    event_type: x.event_type,
                    predicate: x.predicate,
                    subject_surface: x.subject_surface,
                    time_surface: x.time_surface,
                    place_surface: x.place_surface,
                    object_surface: x.object_surface,
                    participant_surfaces: x.participant_surfaces,
                    clause_text: x.clause_text,
                    clause_index: x.clause_index,
                    start_offset: x.clause_start_offset,
                    end_offset: x.clause_end_offset,
                    cross_clause_join: x.cross_clause_join,
                    extractor_id: self.extractor_id().into(),
                    is_posthumous: false,
                    lat: None,
                    lon: None,
                });
            }
            offset += sentence.len() as i32 + 1;
        }
        out
    }
}

fn split_keep(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if ch == '.' || ch == '\n' {
            let t = cur.trim().to_string();
            if !t.is_empty() {
                out.push(t);
            }
            cur.clear();
        }
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}
