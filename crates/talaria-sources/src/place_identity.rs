// crates/talaria-sources/src/place_identity.rs
//! Place identity resolution — separate from geocoding.
//!
//! Grounding chain order:
//!   mention → place identity (aliases / existing entities / authority) → geocode that identity.
//!
//! Coordinates come only from Wikidata P625 / offline gazetteer / wiki page coords.
//! TGN and WHG are identity layers, not coordinate sources; they resolve place names to
//! authoritative identifiers which can then be geocoded via P625.

use serde::{Deserialize, Serialize};

/// A resolved place identity — separate from coordinates.
/// Grounding establishes identity first; geocoding happens after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceIdentity {
    /// Canonical label for the place.
    pub label: String,
    /// Wikidata QID if resolved (e.g., "Q90" for Paris).
    pub wikidata_qid: Option<String>,
    /// Getty TGN identifier if resolved (e.g., "7008038" for Paris).
    pub tgn_id: Option<String>,
    /// World Historical Gazetteer identifier if resolved.
    pub whg_id: Option<String>,
    /// GeoNames ID if resolved.
    pub geonames_id: Option<String>,
    /// Method used to resolve identity: "wikidata_search", "tgn", "whg", "alias_gazetteer", "manual".
    pub identity_source: String,
    /// Confidence score for the identity resolution (0.0 to 1.0).
    pub confidence: f32,
}

impl PlaceIdentity {
    /// Create identity from a Wikidata QID.
    pub fn from_wikidata(label: &str, qid: &str) -> Self {
        Self {
            label: label.to_string(),
            wikidata_qid: Some(qid.to_string()),
            tgn_id: None,
            whg_id: None,
            geonames_id: None,
            identity_source: "wikidata".into(),
            confidence: 0.85,
        }
    }

    /// Create identity from alias gazetteer (no QID, just known name).
    pub fn from_alias(label: &str) -> Self {
        Self {
            label: label.to_string(),
            wikidata_qid: None,
            tgn_id: None,
            whg_id: None,
            geonames_id: None,
            identity_source: "alias_gazetteer".into(),
            confidence: 0.75,
        }
    }

    /// Returns true if this identity has an authoritative QID.
    pub fn has_qid(&self) -> bool {
        self.wikidata_qid.is_some()
    }
}

/// Trait for place identity resolvers.
/// Resolvers establish place identity WITHOUT providing coordinates.
/// After identity is resolved, a separate geocoding step fetches coordinates via P625.
pub trait PlaceIdentityResolver: Send + Sync {
    /// Resolve a place mention to an identity.
    /// Returns None if the place cannot be identified.
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity>;

    /// Name of this resolver for logging/debugging.
    fn name(&self) -> &'static str;
}

/// Getty Thesaurus of Geographic Names (TGN) resolver — stub.
/// TGN provides authoritative place identifiers for art-historical research.
/// Full implementation would use TGN SPARQL endpoint or AAT LOD.
///
/// Note: TGN is an identity layer; it does NOT provide coordinates.
/// After resolving to a TGN ID, coordinates come from Wikidata P625 via sameAs.
pub struct TgnResolver {
    /// Base URL for TGN LOD (when implemented).
    #[allow(dead_code)]
    endpoint: String,
}

impl TgnResolver {
    pub fn new() -> Self {
        Self {
            endpoint: "http://vocab.getty.edu/tgn/".into(),
        }
    }
}

impl Default for TgnResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceIdentityResolver for TgnResolver {
    fn resolve(&self, _mention: &str) -> Option<PlaceIdentity> {
        // Stub: TGN integration requires SPARQL client + alignment to Wikidata.
        // When implemented, this would:
        // 1. Search TGN for place label
        // 2. Return TGN ID + any sameAs Wikidata QID
        // 3. Coordinates come LATER via P625, not from TGN directly
        None
    }

    fn name(&self) -> &'static str {
        "tgn"
    }
}

/// World Historical Gazetteer (WHG) resolver — stub.
/// WHG provides historical place identifiers with temporal scopes.
///
/// Note: WHG is an identity layer; coordinates come from linked Wikidata entities.
pub struct WhgResolver {
    /// Base URL for WHG API (when implemented).
    #[allow(dead_code)]
    endpoint: String,
}

impl WhgResolver {
    pub fn new() -> Self {
        Self {
            endpoint: "https://whgazetteer.org/api/".into(),
        }
    }
}

impl Default for WhgResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceIdentityResolver for WhgResolver {
    fn resolve(&self, _mention: &str) -> Option<PlaceIdentity> {
        // Stub: WHG integration requires REST client + Wikidata alignment.
        // When implemented, this would:
        // 1. Search WHG for place label (with optional temporal scope)
        // 2. Return WHG ID + linked Wikidata QID if available
        // 3. Coordinates come LATER via P625, not from WHG directly
        None
    }

    fn name(&self) -> &'static str {
        "whg"
    }
}

/// Composite resolver that tries multiple identity sources in order.
/// Order: alias gazetteer → TGN → WHG → Wikidata search.
pub struct CompositeIdentityResolver {
    resolvers: Vec<Box<dyn PlaceIdentityResolver>>,
}

impl CompositeIdentityResolver {
    pub fn new() -> Self {
        Self {
            resolvers: vec![
                Box::new(AliasGazetteerResolver),
                Box::new(TgnResolver::new()),
                Box::new(WhgResolver::new()),
            ],
        }
    }

    /// Add a custom resolver to the chain.
    pub fn with_resolver(mut self, resolver: Box<dyn PlaceIdentityResolver>) -> Self {
        self.resolvers.push(resolver);
        self
    }
}

impl Default for CompositeIdentityResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceIdentityResolver for CompositeIdentityResolver {
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        for resolver in &self.resolvers {
            if let Some(identity) = resolver.resolve(mention) {
                return Some(identity);
            }
        }
        None
    }

    fn name(&self) -> &'static str {
        "composite"
    }
}

/// Alias gazetteer resolver — uses the offline alias table.
struct AliasGazetteerResolver;

impl PlaceIdentityResolver for AliasGazetteerResolver {
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        crate::resolve_place_offline(mention).map(|res| PlaceIdentity {
            label: res.label,
            wikidata_qid: res.wikidata_qid,
            tgn_id: None,
            whg_id: None,
            geonames_id: None,
            identity_source: "alias_gazetteer".into(),
            confidence: res.score,
        })
    }

    fn name(&self) -> &'static str {
        "alias_gazetteer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tgn_resolver_is_stub() {
        let resolver = TgnResolver::new();
        assert_eq!(resolver.name(), "tgn");
        // Stub returns None until implemented
        assert!(resolver.resolve("Paris").is_none());
    }

    #[test]
    fn whg_resolver_is_stub() {
        let resolver = WhgResolver::new();
        assert_eq!(resolver.name(), "whg");
        // Stub returns None until implemented
        assert!(resolver.resolve("Constantinople").is_none());
    }

    #[test]
    fn alias_gazetteer_resolves_known_places() {
        let resolver = AliasGazetteerResolver;
        let identity = resolver.resolve("Paris").expect("Paris should resolve");
        assert_eq!(identity.label, "Paris");
        assert_eq!(identity.identity_source, "alias_gazetteer");
    }

    #[test]
    fn composite_resolver_tries_alias_first() {
        let resolver = CompositeIdentityResolver::new();
        let identity = resolver.resolve("Waterloo").expect("Waterloo should resolve");
        assert_eq!(identity.identity_source, "alias_gazetteer");
    }

    #[test]
    fn place_identity_from_wikidata() {
        let identity = PlaceIdentity::from_wikidata("Paris", "Q90");
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q90"));
        assert!(identity.has_qid());
    }
}
