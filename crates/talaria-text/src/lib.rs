// crates/talaria-text/src/lib.rs
pub mod sentences;
pub mod wikitext;

pub use sentences::{segment_wikitext, split_sentences, SentenceSpan};
pub use wikitext::wikitext_to_plain;
