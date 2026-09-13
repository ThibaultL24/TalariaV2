// crates/talaria-sources/src/place_identity.rs
//! Place identity resolution — separate from geocoding.
//!
//! Grounding chain order:
//!   mention → place identity (aliases / existing entities / authority) → geocode that identity.
//!
//! Coordinates come only from Wikidata P625 / offline gazetteer / wiki page coords.
//! TGN and WHG are identity layers, not coordinate sources; they resolve place names to
//! authoritative identifiers which can then be geocoded via P625.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

/// Trait for place identity resolvers (async).
/// Resolvers establish place identity WITHOUT providing coordinates.
/// After identity is resolved, a separate geocoding step fetches coordinates via P625.
///
/// IMPORTANT: This trait is async to support HTTP-based resolvers (TGN SPARQL, WHG REST).
/// Callers must await `resolve()`. Never use `block_on` — it panics in async context.
#[async_trait]
pub trait PlaceIdentityResolver: Send + Sync {
    /// Resolve a place mention to an identity.
    /// Returns None if the place cannot be identified.
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity>;

    /// Name of this resolver for logging/debugging.
    fn name(&self) -> &'static str;
}

/// Getty Thesaurus of Geographic Names (TGN) resolver.
/// TGN provides authoritative place identifiers for art-historical research.
/// Uses the Getty SPARQL endpoint to search for place labels and retrieve TGN IDs.
///
/// Note: TGN is an identity layer; it does NOT provide coordinates.
/// After resolving to a TGN ID, coordinates come from Wikidata P625 via sameAs links.
pub struct TgnResolver {
    http: reqwest::Client,
    endpoint: String,
}

impl TgnResolver {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("TalariaEngine/0.1 (https://github.com/talaria; place-identity resolver)")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            endpoint: "http://vocab.getty.edu/sparql".into(),
        }
    }

    /// Search TGN for a place label and return TGN ID + linked Wikidata QID.
    async fn search_tgn(&self, label: &str) -> Option<(String, Option<String>)> {
        let escaped = sparql_escape(label);
        let query = format!(
            r#"
            PREFIX gvp: <http://vocab.getty.edu/ontology#>
            PREFIX xl: <http://www.w3.org/2008/05/skos-xl#>
            PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
            PREFIX schema: <http://schema.org/>
            SELECT ?subject ?wikidata WHERE {{
                ?subject a gvp:AdminPlaceConcept ;
                         xl:prefLabel/xl:literalForm ?label .
                FILTER(LCASE(STR(?label)) = LCASE("{escaped}"))
                OPTIONAL {{
                    ?subject schema:sameAs ?wikidata .
                    FILTER(STRSTARTS(STR(?wikidata), "http://www.wikidata.org/entity/Q"))
                }}
            }}
            LIMIT 1
            "#
        );

        let response = self
            .http
            .get(&self.endpoint)
            .query(&[("query", &query), ("format", &"json".to_string())])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            tracing::debug!(status = %response.status(), "TGN SPARQL request failed");
            return None;
        }

        let json: serde_json::Value = response.json().await.ok()?;

        let bindings = json
            .pointer("/results/bindings")
            .and_then(|b| b.as_array())?;

        let first = bindings.first()?;

        let tgn_uri = first
            .pointer("/subject/value")
            .and_then(|v| v.as_str())?;

        let tgn_id = tgn_uri
            .rsplit('/')
            .next()
            .map(|s| s.to_string())?;

        let wikidata_qid = first
            .pointer("/wikidata/value")
            .and_then(|v| v.as_str())
            .and_then(|uri| uri.rsplit('/').next())
            .filter(|qid| qid.starts_with('Q'))
            .map(|s| s.to_string());

        Some((tgn_id, wikidata_qid))
    }
}

impl Default for TgnResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlaceIdentityResolver for TgnResolver {
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        let mention = mention.trim();
        if mention.is_empty() {
            return None;
        }

        let (tgn_id, wikidata_qid) = self.search_tgn(mention).await?;
        let has_qid = wikidata_qid.is_some();

        Some(PlaceIdentity {
            label: mention.to_string(),
            wikidata_qid,
            tgn_id: Some(tgn_id),
            whg_id: None,
            geonames_id: None,
            identity_source: "tgn".into(),
            confidence: if has_qid { 0.90 } else { 0.80 },
        })
    }

    fn name(&self) -> &'static str {
        "tgn"
    }
}

fn sparql_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// World Historical Gazetteer (WHG) resolver.
/// WHG provides historical place identifiers with temporal scopes.
/// Uses the WHG REST API to search for place labels.
///
/// Note: WHG is an identity layer; coordinates come from linked Wikidata entities.
pub struct WhgResolver {
    http: reqwest::Client,
    endpoint: String,
    api_token: Option<String>,
}

impl WhgResolver {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("TalariaEngine/0.1 (https://github.com/talaria; place-identity resolver)")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            endpoint: "https://whgazetteer.org/api".into(),
            api_token: std::env::var("WHG_API_TOKEN").ok().filter(|s| !s.is_empty()),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_token.is_some()
    }

    /// Search WHG for a place label and return WHG ID + linked Wikidata QID.
    async fn search_whg(&self, label: &str) -> Option<(String, Option<String>)> {
        let token = self.api_token.as_ref()?;
        let url = format!("{}/places/?q={}", self.endpoint, percent_encode(label));
        
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            tracing::debug!(status = %response.status(), "WHG API request failed");
            return None;
        }

        let json: serde_json::Value = response.json().await.ok()?;

        let features = json
            .get("features")
            .or_else(|| json.get("results"))
            .and_then(|f| f.as_array())?;

        let first = features.first()?;
        let props = first.get("properties")?;

        let whg_id = props
            .get("place_id")
            .and_then(|v| v.as_i64())
            .map(|id| id.to_string())
            .or_else(|| {
                props
                    .get("pid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })?;

        let wikidata_qid = props
            .get("links")
            .and_then(|l| l.as_array())
            .and_then(|links| {
                links.iter().find_map(|link| {
                    let identifier = link.get("identifier")?.as_str()?;
                    if identifier.contains("wikidata.org") || identifier.starts_with('Q') {
                        let qid = identifier.rsplit('/').next().unwrap_or(identifier);
                        if qid.starts_with('Q') {
                            return Some(qid.to_string());
                        }
                    }
                    None
                })
            });

        Some((whg_id, wikidata_qid))
    }
}

impl Default for WhgResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlaceIdentityResolver for WhgResolver {
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        let mention = mention.trim();
        if mention.is_empty() || !self.is_configured() {
            return None;
        }

        let (whg_id, wikidata_qid) = self.search_whg(mention).await?;
        let has_qid = wikidata_qid.is_some();

        Some(PlaceIdentity {
            label: mention.to_string(),
            wikidata_qid,
            tgn_id: None,
            whg_id: Some(whg_id),
            geonames_id: None,
            identity_source: "whg".into(),
            confidence: if has_qid { 0.88 } else { 0.78 },
        })
    }

    fn name(&self) -> &'static str {
        "whg"
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Composite resolver that tries multiple identity sources in order.
/// Order: alias gazetteer → TGN → WHG.
///
/// IMPORTANT: If alias gazetteer resolves but has no QID, we continue trying
/// TGN/WHG to get a QID and merge results. Coordinates come from alias gazetteer
/// (or page coords), identity comes from TGN/WHG. This ensures `place_identity_qid`
/// gets populated even for places in our offline alias table.
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

#[async_trait]
impl PlaceIdentityResolver for CompositeIdentityResolver {
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        let mut best_identity: Option<PlaceIdentity> = None;

        for resolver in &self.resolvers {
            if let Some(identity) = resolver.resolve(mention).await {
                // If we have no identity yet, take this one
                if best_identity.is_none() {
                    best_identity = Some(identity.clone());
                    // If this identity already has a QID, we're done
                    if identity.has_qid() {
                        return best_identity;
                    }
                    // Otherwise, continue trying other resolvers to find a QID
                    continue;
                }

                // If we already have an identity without QID, and this one has a QID,
                // merge them: keep original source info, add the QID and authority IDs
                if let Some(ref mut best) = best_identity {
                    if !best.has_qid() && identity.has_qid() {
                        best.wikidata_qid = identity.wikidata_qid.clone();
                        best.tgn_id = best.tgn_id.clone().or(identity.tgn_id.clone());
                        best.whg_id = best.whg_id.clone().or(identity.whg_id.clone());
                        best.geonames_id = best.geonames_id.clone().or(identity.geonames_id.clone());
                        // Upgrade confidence if QID found via authority
                        if identity.confidence > best.confidence {
                            best.confidence = identity.confidence;
                        }
                        // Note which source provided the QID
                        best.identity_source = format!("{}+{}", best.identity_source, identity.identity_source);
                        tracing::debug!(
                            mention = mention,
                            qid = ?best.wikidata_qid,
                            source = %best.identity_source,
                            "merged identity from multiple sources"
                        );
                        return best_identity;
                    }
                }
            }
        }

        best_identity
    }

    fn name(&self) -> &'static str {
        "composite"
    }
}

/// Alias gazetteer resolver — uses the offline alias table.
struct AliasGazetteerResolver;

#[async_trait]
impl PlaceIdentityResolver for AliasGazetteerResolver {
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
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

    /// Mock resolver that returns identity without QID.
    struct NoQidResolver;

    #[async_trait]
    impl PlaceIdentityResolver for NoQidResolver {
        async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
            Some(PlaceIdentity {
                label: mention.to_string(),
                wikidata_qid: None,
                tgn_id: None,
                whg_id: None,
                geonames_id: None,
                identity_source: "no_qid_resolver".into(),
                confidence: 0.8,
            })
        }

        fn name(&self) -> &'static str {
            "no_qid_resolver"
        }
    }

    /// Mock resolver that returns identity WITH a QID.
    struct QidResolver {
        qid: String,
    }

    #[async_trait]
    impl PlaceIdentityResolver for QidResolver {
        async fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
            Some(PlaceIdentity {
                label: mention.to_string(),
                wikidata_qid: Some(self.qid.clone()),
                tgn_id: Some("TGN123".into()),
                whg_id: None,
                geonames_id: None,
                identity_source: "qid_resolver".into(),
                confidence: 0.9,
            })
        }

        fn name(&self) -> &'static str {
            "qid_resolver"
        }
    }

    #[tokio::test]
    async fn tgn_resolver_basic() {
        let resolver = TgnResolver::new();
        assert_eq!(resolver.name(), "tgn");
    }

    #[tokio::test]
    async fn whg_resolver_basic() {
        let resolver = WhgResolver::new();
        assert_eq!(resolver.name(), "whg");
    }

    #[tokio::test]
    async fn alias_gazetteer_resolves_known_places() {
        let resolver = AliasGazetteerResolver;
        let identity = resolver.resolve("Paris").await.expect("Paris should resolve");
        assert_eq!(identity.label, "Paris");
        assert_eq!(identity.identity_source, "alias_gazetteer");
    }

    #[tokio::test]
    async fn composite_resolver_tries_alias_first() {
        let resolver = CompositeIdentityResolver::new();
        let identity = resolver.resolve("Waterloo").await.expect("Waterloo should resolve");
        assert_eq!(identity.identity_source, "alias_gazetteer");
    }

    #[test]
    fn place_identity_from_wikidata() {
        let identity = PlaceIdentity::from_wikidata("Paris", "Q90");
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q90"));
        assert!(identity.has_qid());
    }

    /// Regression test: resolving place identity from async context must not panic.
    /// This verifies the fix for the block_on panic when called inside async runtime.
    #[tokio::test]
    async fn resolve_from_async_context_no_panic() {
        let resolver = CompositeIdentityResolver::new();
        
        // Multiple concurrent resolutions should work without panic
        let results = tokio::join!(
            resolver.resolve("Paris"),
            resolver.resolve("London"),
            resolver.resolve("Waterloo"),
        );

        // At least Paris should resolve via alias gazetteer
        assert!(results.0.is_some() || results.1.is_some() || results.2.is_some());
    }

    /// Regression test: spawned task resolution must not panic.
    #[tokio::test]
    async fn resolve_in_spawned_task_no_panic() {
        let handle = tokio::spawn(async {
            let resolver = CompositeIdentityResolver::new();
            resolver.resolve("Paris").await
        });

        let result = handle.await.expect("spawned task should complete");
        assert!(result.is_some());
    }

    /// Regression test: composite resolver continues trying TGN/WHG when alias gazetteer lacks QID.
    /// This is the fix for PR #23/#25 where place_identity_qid = 0%.
    #[tokio::test]
    async fn composite_merges_qid_from_second_resolver() {
        // Build a custom composite: no_qid first, then qid resolver
        let resolver = CompositeIdentityResolver {
            resolvers: vec![
                Box::new(NoQidResolver),
                Box::new(QidResolver { qid: "Q90".into() }),
            ],
        };

        let identity = resolver
            .resolve("Test Place")
            .await
            .expect("should resolve");

        // Should have the label from first resolver
        assert_eq!(identity.label, "Test Place");
        // Should have the QID merged from second resolver
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q90"));
        // Should have the TGN ID from second resolver
        assert_eq!(identity.tgn_id.as_deref(), Some("TGN123"));
        // Source should show the merge
        assert!(
            identity.identity_source.contains("no_qid_resolver"),
            "should include first resolver source"
        );
        assert!(
            identity.identity_source.contains("qid_resolver"),
            "should include second resolver source"
        );
    }

    /// Test: when first resolver already has QID, don't continue trying others.
    #[tokio::test]
    async fn composite_stops_early_when_qid_found() {
        let resolver = CompositeIdentityResolver {
            resolvers: vec![
                Box::new(QidResolver { qid: "Q100".into() }),
                Box::new(QidResolver { qid: "Q200".into() }),
            ],
        };

        let identity = resolver
            .resolve("Test Place")
            .await
            .expect("should resolve");

        // Should have the QID from first resolver (stopped early)
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q100"));
        // Source should only be from first resolver
        assert_eq!(identity.identity_source, "qid_resolver");
    }

    /// Test: alias gazetteer returns coords but no QID, TGN provides QID.
    #[tokio::test]
    async fn alias_gazetteer_coords_tgn_qid_merge() {
        // Use real alias gazetteer which has coords but no QID for most places
        let resolver = CompositeIdentityResolver {
            resolvers: vec![
                Box::new(AliasGazetteerResolver),
                Box::new(QidResolver { qid: "Q90".into() }),
            ],
        };

        // Paris is in alias gazetteer but alias gazetteer returns wikidata_qid: None
        let identity = resolver.resolve("Paris").await.expect("should resolve");

        // Alias gazetteer doesn't have QID, so we should get one from QidResolver
        // Note: The alias gazetteer in resolve_place_offline returns wikidata_qid: None
        if identity.wikidata_qid.is_none() {
            // If alias gazetteer didn't short-circuit, we'd have a QID
            // This test confirms the merge behavior when alias has no QID
            panic!("Expected QID to be merged from second resolver");
        }
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q90"));
    }
}
