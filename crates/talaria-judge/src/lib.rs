// crates/talaria-judge/src/lib.rs
mod claims;
mod place;
mod rules;
mod time;

pub use claims::{classify_claim_text, ClaimClass};
pub use place::{parse_place_surface, ParsedPlace};
pub use rules::{judge_candidate, CandidateInput, JudgeLabel, JudgeVerdict};
pub use time::{parse_time_surface, ParsedTime};
