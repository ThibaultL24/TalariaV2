// crates/talaria-judge/src/lib.rs
mod claims;
mod dump_mine;
mod place;
mod rules;
mod time;

pub use claims::{classify_claim_text, ClaimClass};
pub use dump_mine::{
    death_refers_to_other_person, mine_sentence, mine_sentence_with_carry, split_heading_chunks,
    MineCarry, MinedCandidate, EXTRACTOR_ANECDOTE, EXTRACTOR_KEYWORDS,
};
pub use place::{find_place_in_text, parse_place_surface, ParsedPlace};
pub use rules::{judge_candidate, CandidateInput, JudgeLabel, JudgeVerdict};
pub use time::{parse_time_surface, ParsedTime};
