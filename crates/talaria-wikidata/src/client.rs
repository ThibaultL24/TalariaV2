// crates/talaria-wikidata/src/client.rs
use anyhow::Result;
use serde_json::Value;

const USER_AGENT: &str = "TalariaEngine/0.1 (https://github.com/talaria; geocoding bot)";

#[derive(Debug, Clone)]
pub struct WikidataSearchHit {
    pub qid: String,
    pub label: String,
    pub description: Option<String>,
}

pub struct WikidataClient {
    http: reqwest::Client,
    api_base: String,
}

impl WikidataClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self {
            http,
            api_base: "https://www.wikidata.org/w/api.php".into(),
        })
    }

    pub async fn search_entity(&self, label: &str, lang: &str) -> Result<Option<String>> {
        Ok(self
            .search_entities(label, lang, 1)
            .await?
            .into_iter()
            .next()
            .map(|hit| hit.qid))
    }

    pub async fn search_entities(
        &self,
        label: &str,
        lang: &str,
        limit: u32,
    ) -> Result<Vec<WikidataSearchHit>> {
        let response = self
            .http
            .get(&self.api_base)
            .query(&[
                ("action", "wbsearchentities"),
                ("search", label),
                ("language", lang),
                ("format", "json"),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let hits = response
            .get("search")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let qid = item.get("id")?.as_str()?.to_string();
                        let label = item
                            .get("label")
                            .and_then(|value| value.as_str())
                            .unwrap_or(&qid)
                            .to_string();
                        let description = item
                            .get("description")
                            .and_then(|value| value.as_str())
                            .map(str::to_string);
                        Some(WikidataSearchHit {
                            qid,
                            label,
                            description,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(hits)
    }

    pub async fn fetch_coordinates(&self, qid: &str) -> Result<Option<(f64, f64)>> {
        let response = self
            .http
            .get(&self.api_base)
            .query(&[
                ("action", "wbgetentities"),
                ("ids", qid),
                ("props", "claims"),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let Some(coord) = response
            .pointer(&format!("/entities/{qid}/claims/P625/0/mainsnak/datavalue/value"))
        else {
            return Ok(None);
        };

        let Some(lat) = coord.get("latitude").and_then(|v| v.as_f64()) else {
            return Ok(None);
        };
        let Some(lon) = coord.get("longitude").and_then(|v| v.as_f64()) else {
            return Ok(None);
        };

        Ok(Some((lat, lon)))
    }
}

impl Default for WikidataClient {
    fn default() -> Self {
        Self::new().expect("wikidata client")
    }
}
