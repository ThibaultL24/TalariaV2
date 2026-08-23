// crates/talaria-api/src/llm.rs
//! OpenAI display-layer client. Never writes canonical_events.

use serde_json::{json, Value};

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
const DEFAULT_MODEL: &str = "gpt-5.4";

pub fn api_key() -> Option<String> {
    std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

pub fn model() -> String {
    std::env::var("OPENAI_MODEL")
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

pub fn is_configured() -> bool {
    api_key().is_some()
}

pub struct PingResult {
    pub ok: bool,
    pub model: String,
    pub latency_ms: u128,
    pub error: Option<String>,
}

/// Tiny round-trip so we know the v1 project key + model actually answer.
pub async fn ping() -> PingResult {
    let model = model();
    let Some(key) = api_key() else {
        return PingResult {
            ok: false,
            model,
            latency_ms: 0,
            error: Some("OPENAI_API_KEY missing".into()),
        };
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return PingResult {
                ok: false,
                model,
                latency_ms: 0,
                error: Some(e.to_string()),
            };
        }
    };

    let started = std::time::Instant::now();
    let response = client
        .post(OPENAI_RESPONSES_URL)
        .bearer_auth(key)
        .json(&json!({
            "model": model,
            "input": "Reply with the single word OK.",
            "store": false,
        }))
        .send()
        .await;
    let latency_ms = started.elapsed().as_millis();

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body: Value = resp.json().await.unwrap_or(json!({}));
            if status.is_success() {
                PingResult {
                    ok: true,
                    model,
                    latency_ms,
                    error: None,
                }
            } else {
                let message = body
                    .pointer("/error/message")
                    .and_then(|v| v.as_str())
                    .unwrap_or(status.as_str())
                    .to_string();
                PingResult {
                    ok: false,
                    model,
                    latency_ms,
                    error: Some(message),
                }
            }
        }
        Err(e) => PingResult {
            ok: false,
            model,
            latency_ms,
            error: Some(e.to_string()),
        },
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LlmExtractItem {
    #[serde(default)]
    pub lane: String,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub role: String,
    pub year: Option<i32>,
    pub place_surface: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(alias = "quote", alias = "quoted_text")]
    pub quoted_text: String,
    #[serde(default)]
    pub confidence: f64,
}

pub fn parse_extract_items(payload: &str) -> Vec<LlmExtractItem> {
    let trimmed = payload.trim();
    let json_slice = if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        &trimmed[start..=end]
    } else {
        trimmed
    };
    serde_json::from_str::<Vec<LlmExtractItem>>(json_slice).unwrap_or_default()
}

impl LlmExtractItem {
    pub fn into_raw(self) -> talaria_quality::RawExtractItem {
        talaria_quality::RawExtractItem {
            lane: self.lane,
            event_type: if self.event_type.is_empty() {
                "historical_fact".into()
            } else {
                self.event_type
            },
            role: if self.role.is_empty() {
                "direct".into()
            } else {
                self.role
            },
            year: self.year,
            place_surface: self.place_surface,
            summary: self.summary,
            quoted_text: self.quoted_text,
            confidence: if self.confidence == 0.0 {
                0.7
            } else {
                self.confidence
            },
        }
    }
}

pub async fn extract_chunk(
    subject: &str,
    page_title: &str,
    chunk: &str,
) -> anyhow::Result<Vec<LlmExtractItem>> {
    let Some(key) = api_key() else {
        anyhow::bail!("OPENAI_API_KEY missing");
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()?;
    let prompt = format!(
        "Subject: {subject}\nPage: {page_title}\n\n\
         Extract JSON array of items about THIS subject only.\n\
         Each item: lane (fact|debate), event_type (birth,death,residence,travel,battle,treaty,diplomatic,office,education,work,anecdote,commemoration,other), \
         role (direct|indirect), year (number or null), place_surface, summary, quoted_text (exact substring of the text), confidence 0-1.\n\
         Facts: every dated or located event about the subject — life, work, travel, AND commemorations (statue, plaque, tomb, museum, school named after them).\n\
         place_surface MUST be a named city, town, or institution (Warsaw, Paris, Sorbonne), never 'her house', 'the institute', or a country alone.\n\
         Extract as many grounded facts as the text supports. Debates: controversies, theses, attribution disputes. Never invent quotes.\n\
         Text:\n{chunk}"
    );
    let response = client
        .post(OPENAI_RESPONSES_URL)
        .bearer_auth(key)
        .json(&json!({
            "model": model(),
            "input": prompt,
            "store": false,
        }))
        .send()
        .await?;
    let body: Value = response.json().await.unwrap_or(json!({}));
    let text = output_text(&body);
    Ok(parse_extract_items(&text))
}

fn output_text(body: &Value) -> String {
    if let Some(s) = body.get("output_text").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    body.get("output")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                item.get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|parts| {
                        parts.iter().find_map(|p| {
                            p.get("text")
                                .and_then(|t| t.as_str())
                                .map(str::to_string)
                        })
                    })
            })
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_array_of_extracts() {
        let raw = r#"[{"lane":"fact","event_type":"birth","quoted_text":"born in Warsaw","year":1867,"place_surface":"Warsaw","summary":"birth","confidence":0.9}]"#;
        let items = parse_extract_items(raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].place_surface.as_deref(), Some("Warsaw"));
    }
}
