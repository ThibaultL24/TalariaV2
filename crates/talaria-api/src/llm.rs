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
