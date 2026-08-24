// crates/talaria-text/src/lib.rs
pub mod fragments;
pub mod infobox;
pub mod sections;
pub mod sentences;
pub mod wikitext;

pub use fragments::{
    extract_refs, fragment_wikitext, FragmentCitation, FragmentLink, WikiContentFragment,
};
pub use infobox::{
    extract_wikilinks, infobox_life_facts, parse_infobox_fields, InfoboxField, InfoboxLifeFacts,
    WikiLink,
};
pub use sections::{split_wiki_sections, WikiSectionSpan};
pub use sentences::{segment_wikitext, split_sentences, SentenceSpan};
pub use wikitext::wikitext_to_plain;
