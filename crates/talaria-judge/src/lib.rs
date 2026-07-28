// crates/talaria-judge/src/lib.rs
mod place;
mod rules;
mod time;

pub use place::{parse_place_surface, ParsedPlace};
pub use rules::{judge_candidate, CandidateInput, JudgeLabel, JudgeVerdict};
pub use time::{parse_time_surface, ParsedTime};
