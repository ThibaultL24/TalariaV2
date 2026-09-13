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
use std::sync::OnceLock;
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

/// Getty Thesaurus of Geographic Names (TGN) resolver.
/// TGN provides authoritative place identifiers for art-historical research.
/// Uses TGN SPARQL endpoint at vocab.getty.edu.
///
/// Note: TGN is an identity layer; it does NOT provide coordinates.
/// After resolving to a TGN ID, coordinates come from Wikidata P625 via sameAs.
pub struct TgnResolver {
    endpoint: String,
    http: OnceLock<reqwest::Client>,
}

impl TgnResolver {
    pub fn new() -> Self {
        Self {
            endpoint: "http://vocab.getty.edu/sparql".into(),
            http: OnceLock::new(),
        }
    }

    fn get_http(&self) -> &reqwest::Client {
        self.http.get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("TalariaEngine/0.1 (+grounding; tgn)")
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
    }

    /// Synchronous wrapper for async SPARQL query.
    fn query_tgn_sync(&self, mention: &str) -> Option<PlaceIdentity> {
        let rt = tokio::runtime::Handle::try_current().ok()?;
        rt.block_on(self.query_tgn(mention))
    }

    async fn query_tgn(&self, mention: &str) -> Option<PlaceIdentity> {
        let query = tgn_place_query(mention);
        let http = self.get_http();

        let resp = http
            .get(&self.endpoint)
            .query(&[("query", &query), ("format", &"application/sparql-results+json".to_string())])
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let payload: serde_json::Value = resp.json().await.ok()?;
        parse_tgn_sparql_result(&payload, mention)
    }
}

impl Default for TgnResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceIdentityResolver for TgnResolver {
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        // Try async if runtime available, otherwise return None (stub behavior in sync context)
        self.query_tgn_sync(mention)
    }

    fn name(&self) -> &'static str {
        "tgn"
    }
}

fn tgn_place_query(mention: &str) -> String {
    let escaped = sparql_escape(mention);
    format!(
        r#"PREFIX gvp: <http://vocab.getty.edu/ontology#>
PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
PREFIX skosxl: <http://www.w3.org/2008/05/skos-xl#>
PREFIX schema: <http://schema.org/>

SELECT ?subject ?label ?wikidata WHERE {{
  ?subject a gvp:Subject ;
           skos:inScheme <http://vocab.getty.edu/tgn/> ;
           gvp:prefLabelGVP/skosxl:literalForm ?label .
  OPTIONAL {{
    ?subject schema:sameAs ?wikidata .
    FILTER(STRSTARTS(STR(?wikidata), "http://www.wikidata.org/entity/Q"))
  }}
  FILTER(LCASE(?label) = LCASE("{escaped}"))
}}
LIMIT 5"#
    )
}

fn parse_tgn_sparql_result(payload: &serde_json::Value, mention: &str) -> Option<PlaceIdentity> {
    let bindings = payload.pointer("/results/bindings")?.as_array()?;
    let row = bindings.first()?;

    let tgn_uri = row.get("subject")?.get("value")?.as_str()?;
    let tgn_id = tgn_uri.rsplit('/').next()?.to_string();
    let label = row
        .get("label")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or(mention)
        .to_string();

    let wikidata_qid = row
        .get("wikidata")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .and_then(|uri| uri.rsplit('/').next())
        .filter(|qid| qid.starts_with('Q'))
        .map(str::to_string);

    let has_qid = wikidata_qid.is_some();
    Some(PlaceIdentity {
        label,
        wikidata_qid,
        tgn_id: Some(tgn_id),
        whg_id: None,
        geonames_id: None,
        identity_source: "tgn".into(),
        confidence: if has_qid { 0.90 } else { 0.80 },
    })
}

fn sparql_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// World Historical Gazetteer (WHG) resolver.
/// WHG provides historical place identifiers with temporal scopes.
/// Requires WHG_API_TOKEN environment variable for authentication.
///
/// Note: WHG is an identity layer; coordinates come from linked Wikidata entities.
pub struct WhgResolver {
    endpoint: String,
    api_token: Option<String>,
    http: OnceLock<reqwest::Client>,
}

impl WhgResolver {
    pub fn new() -> Self {
        Self {
            endpoint: "https://whgazetteer.org/api".into(),
            api_token: std::env::var("WHG_API_TOKEN").ok().filter(|s| !s.is_empty()),
            http: OnceLock::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_token.is_some()
    }

    fn get_http(&self) -> &reqwest::Client {
        self.http.get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("TalariaEngine/0.1 (+grounding; whg)")
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
    }

    /// Synchronous wrapper for async API query.
    fn query_whg_sync(&self, mention: &str) -> Option<PlaceIdentity> {
        let rt = tokio::runtime::Handle::try_current().ok()?;
        rt.block_on(self.query_whg(mention))
    }

    async fn query_whg(&self, mention: &str) -> Option<PlaceIdentity> {
        let token = self.api_token.as_ref()?;
        let http = self.get_http();

        let url = format!("{}/places/?q={}", self.endpoint, percent_encode(mention));
        let resp = http
            .get(&url)
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let payload: serde_json::Value = resp.json().await.ok()?;
        parse_whg_result(&payload, mention)
    }
}

impl Default for WhgResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceIdentityResolver for WhgResolver {
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity> {
        if !self.is_configured() {
            return None;
        }
        self.query_whg_sync(mention)
    }

    fn name(&self) -> &'static str {
        "whg"
    }
}

fn parse_whg_result(payload: &serde_json::Value, mention: &str) -> Option<PlaceIdentity> {
    let features = payload.get("features").or_else(|| payload.get("results"))?.as_array()?;
    let feature = features.first()?;
    let props = feature.get("properties")?;

    let whg_id = props.get("place_id")?.to_string().trim_matches('"').to_string();
    let label = props
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(mention)
        .to_string();

    // Extract Wikidata QID from links if available
    let wikidata_qid = props
        .get("links")
        .and_then(|v| v.as_array())
        .and_then(|links| {
            links.iter().find_map(|link| {
                let id = link.get("identifier")?.as_str()?;
                if id.contains("wikidata.org") || id.starts_with('Q') {
                    let qid = id.rsplit('/').next().unwrap_or(id);
                    if qid.starts_with('Q') {
                        return Some(qid.to_string());
                    }
                }
                None
            })
        });

    let has_qid = wikidata_qid.is_some();
    Some(PlaceIdentity {
        label,
        wikidata_qid,
        tgn_id: None,
        whg_id: Some(whg_id),
        geonames_id: None,
        identity_source: "whg".into(),
        confidence: if has_qid { 0.88 } else { 0.78 },
    })
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
    fn tgn_resolver_returns_none_without_runtime() {
        let resolver = TgnResolver::new();
        assert_eq!(resolver.name(), "tgn");
        // Returns None in test context without tokio runtime
        assert!(resolver.resolve("Paris").is_none());
    }

    #[test]
    fn whg_resolver_requires_token() {
        let resolver = WhgResolver::new();
        assert_eq!(resolver.name(), "whg");
        // Returns None without WHG_API_TOKEN configured
        if !resolver.is_configured() {
            assert!(resolver.resolve("Constantinople").is_none());
        }
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

    #[test]
    fn tgn_sparql_query_format() {
        let query = tgn_place_query("Paris");
        assert!(query.contains("vocab.getty.edu/tgn"));
        assert!(query.contains("LCASE(\"Paris\")"));
    }

    #[test]
    fn parse_tgn_result_extracts_qid() {
        let payload = serde_json::json!({
            "results": {
                "bindings": [{
                    "subject": { "value": "http://vocab.getty.edu/tgn/7008038" },
                    "label": { "value": "Paris" },
                    "wikidata": { "value": "http://www.wikidata.org/entity/Q90" }
                }]
            }
        });
        let identity = parse_tgn_sparql_result(&payload, "Paris").unwrap();
        assert_eq!(identity.tgn_id.as_deref(), Some("7008038"));
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q90"));
        assert_eq!(identity.identity_source, "tgn");
    }

    #[test]
    fn parse_whg_result_extracts_links() {
        let payload = serde_json::json!({
            "features": [{
                "properties": {
                    "place_id": 12345,
                    "title": "Constantinople",
                    "links": [
                        { "type": "closeMatch", "identifier": "https://www.wikidata.org/entity/Q16869" }
                    ]
                }
            }]
        });
        let identity = parse_whg_result(&payload, "Constantinople").unwrap();
        assert_eq!(identity.whg_id.as_deref(), Some("12345"));
        assert_eq!(identity.wikidata_qid.as_deref(), Some("Q16869"));
        assert_eq!(identity.identity_source, "whg");
    }
}
