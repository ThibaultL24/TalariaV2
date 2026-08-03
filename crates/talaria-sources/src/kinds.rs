// crates/talaria-sources/src/kinds.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Wikidata,
    Wikipedia,
    WikimediaCommons,
    Wikisource,
    Bnf,
    Gallica,
    Europeana,
    OpenLibrary,
    InternetArchive,
    Persee,
    Viaf,
    Isni,
    IdRef,
    Fixture,
    Other(String),
}

impl SourceKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Wikidata => "wikidata",
            Self::Wikipedia => "wikipedia",
            Self::WikimediaCommons => "wikimedia_commons",
            Self::Wikisource => "wikisource",
            Self::Bnf => "bnf",
            Self::Gallica => "gallica",
            Self::Europeana => "europeana",
            Self::OpenLibrary => "open_library",
            Self::InternetArchive => "internet_archive",
            Self::Persee => "persee",
            Self::Viaf => "viaf",
            Self::Isni => "isni",
            Self::IdRef => "idref",
            Self::Fixture => "fixture",
            Self::Other(s) => s.as_str(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "wikidata" => Self::Wikidata,
            "wikipedia" | "wikipedia_en" | "wikipedia_fr" => Self::Wikipedia,
            "wikimedia_commons" | "commons" => Self::WikimediaCommons,
            "wikisource" => Self::Wikisource,
            "bnf" | "data.bnf.fr" => Self::Bnf,
            "gallica" => Self::Gallica,
            "europeana" => Self::Europeana,
            "open_library" | "openlibrary" => Self::OpenLibrary,
            "internet_archive" | "archive.org" | "ia" => Self::InternetArchive,
            "persee" => Self::Persee,
            "viaf" => Self::Viaf,
            "isni" => Self::Isni,
            "idref" => Self::IdRef,
            "fixture" => Self::Fixture,
            other => Self::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAccessMode {
    Api,
    Dump,
    OaiPmh,
    Iiif,
    Rdf,
    Sparql,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Article,
    StructuredStatement,
    AuthorityRecord,
    BibliographicNotice,
    BookOcr,
    PressOcr,
    Manuscript,
    MediaCaption,
    ChronologyList,
    Table,
    Correspondence,
    AcademicArticle,
    Other(String),
}

impl DocumentType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Article => "article",
            Self::StructuredStatement => "structured_statement",
            Self::AuthorityRecord => "authority_record",
            Self::BibliographicNotice => "bibliographic_notice",
            Self::BookOcr => "book_ocr",
            Self::PressOcr => "press_ocr",
            Self::Manuscript => "manuscript",
            Self::MediaCaption => "media_caption",
            Self::ChronologyList => "chronology_list",
            Self::Table => "table",
            Self::Correspondence => "correspondence",
            Self::AcademicArticle => "academic_article",
            Self::Other(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMethod {
    SubjectSearch,
    LinkedEntity,
    IdentifierLookup,
    Sparql,
    CatalogSearch,
    Fixture,
}

impl DiscoveryMethod {
    pub fn as_str(&self) -> &str {
        match self {
            Self::SubjectSearch => "subject_search",
            Self::LinkedEntity => "linked_entity",
            Self::IdentifierLookup => "identifier_lookup",
            Self::Sparql => "sparql",
            Self::CatalogSearch => "catalog_search",
            Self::Fixture => "fixture",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCapabilities {
    pub access_mode: SourceAccessMode,
    pub provides_text: bool,
    pub provides_structured_statements: bool,
    pub provides_coordinates: bool,
    pub provides_identifiers: bool,
    pub license_notes: String,
    pub default_confidence_structured: f32,
    pub default_confidence_ocr: f32,
    pub identifiers: Vec<String>,
    pub document_types: Vec<DocumentType>,
}
