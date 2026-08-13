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
    ThesesFr,
    Hal,
    Crossref,
    OpenAlex,
    OpenEdition,
    Sudoc,
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
            Self::ThesesFr => "theses_fr",
            Self::Hal => "hal",
            Self::Crossref => "crossref",
            Self::OpenAlex => "open_alex",
            Self::OpenEdition => "open_edition",
            Self::Sudoc => "sudoc",
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
            "theses_fr" | "theses.fr" | "theses" => Self::ThesesFr,
            "hal" | "hal_shs" => Self::Hal,
            "crossref" => Self::Crossref,
            "open_alex" | "openalex" => Self::OpenAlex,
            "open_edition" | "openedition" => Self::OpenEdition,
            "sudoc" => Self::Sudoc,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityTier {
    Institutional,
    AcademicPublisher,
    ScholarlyIndex,
    HeritageAggregator,
    CommunityCatalog,
}

impl AuthorityTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Institutional => "institutional",
            Self::AcademicPublisher => "academic_publisher",
            Self::ScholarlyIndex => "scholarly_index",
            Self::HeritageAggregator => "heritage_aggregator",
            Self::CommunityCatalog => "community_catalog",
        }
    }
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
    Thesis,
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
            Self::Thesis => "thesis",
            Self::Other(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcademicStatus {
    PeerReviewed,
    DoctoralDefended,
    AcademicUnreviewed,
    PrimarySource,
    CatalogRecord,
    Unknown,
}

impl AcademicStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PeerReviewed => "peer_reviewed",
            Self::DoctoralDefended => "doctoral_defended",
            Self::AcademicUnreviewed => "academic_unreviewed",
            Self::PrimarySource => "primary_source",
            Self::CatalogRecord => "catalog_record",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "peer_reviewed" => Self::PeerReviewed,
            "doctoral_defended" => Self::DoctoralDefended,
            "academic_unreviewed" => Self::AcademicUnreviewed,
            "primary_source" => Self::PrimarySource,
            "catalog_record" => Self::CatalogRecord,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Open,
    Restricted,
    MetadataOnly,
    Unknown,
}

impl AccessLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Restricted => "restricted",
            Self::MetadataOnly => "metadata_only",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "open" => Self::Open,
            "restricted" => Self::Restricted,
            "metadata_only" => Self::MetadataOnly,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionRole {
    Author,
    ThesisAdvisor,
    JuryMember,
    JuryPresident,
    Rapporteur,
    Institution,
    DoctoralSchool,
    CotutelleInstitution,
    ResearchPartner,
    Editor,
    Publisher,
    Other,
}

impl ContributionRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::ThesisAdvisor => "thesis_advisor",
            Self::JuryMember => "jury_member",
            Self::JuryPresident => "jury_president",
            Self::Rapporteur => "rapporteur",
            Self::Institution => "institution",
            Self::DoctoralSchool => "doctoral_school",
            Self::CotutelleInstitution => "cotutelle_institution",
            Self::ResearchPartner => "research_partner",
            Self::Editor => "editor",
            Self::Publisher => "publisher",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentifierScheme {
    Nnt,
    Ppn,
    Doi,
    Isbn10,
    Isbn13,
    Ark,
    HalId,
    NumSujet,
    Oclc,
    Olid,
    Other,
}

impl IdentifierScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nnt => "nnt",
            Self::Ppn => "ppn",
            Self::Doi => "doi",
            Self::Isbn10 => "isbn10",
            Self::Isbn13 => "isbn13",
            Self::Ark => "ark",
            Self::HalId => "hal_id",
            Self::NumSujet => "num_sujet",
            Self::Oclc => "oclc",
            Self::Olid => "olid",
            Self::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "nnt" => Self::Nnt,
            "ppn" => Self::Ppn,
            "doi" => Self::Doi,
            "isbn10" => Self::Isbn10,
            "isbn13" => Self::Isbn13,
            "ark" => Self::Ark,
            "hal_id" => Self::HalId,
            "num_sujet" => Self::NumSujet,
            "oclc" => Self::Oclc,
            "olid" => Self::Olid,
            _ => Self::Other,
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
    pub authority_tier: AuthorityTier,
    pub provides_text: bool,
    pub provides_structured_statements: bool,
    pub provides_coordinates: bool,
    pub provides_identifiers: bool,
    pub provides_full_text: bool,
    pub provides_ocr: bool,
    pub provides_iiif: bool,
    pub provides_audiovisual: bool,
    pub provides_authority_alignment: bool,
    pub license_notes: String,
    pub default_confidence_structured: f32,
    pub default_confidence_ocr: f32,
    pub identifiers: Vec<String>,
    pub document_types: Vec<DocumentType>,
}
