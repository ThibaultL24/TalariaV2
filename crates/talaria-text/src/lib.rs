// crates/talaria-text/src/lib.rs
pub mod sections;
pub mod sentences;
pub mod wikitext;

pub use sections::{split_wiki_sections, WikiSectionSpan};
pub use sentences::{segment_wikitext, split_sentences, SentenceSpan};
pub use wikitext::wikitext_to_plain;
