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
    let text = output_text(&body).unwrap_or_default();
    Ok(parse_extract_items(&text))
}

pub fn judge_enabled() -> bool {
    is_configured()
        && std::env::var("TALARIA_LLM_JUDGE")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"))
            .unwrap_or(true)
}

/// Overlay judge: may drop, un-place, or relabel. Never adds events or coordinates.
pub async fn judge_raw_candidates(
    subject: &str,
    occupations: &[String],
    raws: Vec<talaria_sources::extractors::RawCandidate>,
) -> Vec<talaria_sources::extractors::RawCandidate> {
    if raws.is_empty() {
        return raws;
    }
    let Some(key) = api_key() else {
        return raws;
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "llm overlay client failed — keeping deterministic extracts");
            return raws;
        }
    };
    let model = model();
    let occ = occupations.join(", ");
    let mut out = Vec::with_capacity(raws.len());
    for chunk in raws.chunks(12) {
        let items: Vec<talaria_quality::OverlayItem> = chunk
            .iter()
            .enumerate()
            .map(|(i, r)| talaria_quality::OverlayItem {
                i,
                event_type: r.event_type.clone(),
                year: r.time_surface.as_deref().and_then(first_year),
                place: r.place_surface.clone(),
                clause: r.clause_text.chars().take(400).collect(),
            })
            .collect();
        match judge_chunk(&client, &key, &model, subject, &occ, &items).await {
            Ok(verdicts) => {
                let by_i: std::collections::HashMap<usize, talaria_quality::OverlayVerdict> =
                    verdicts.into_iter().map(|v| (v.i, v)).collect();
                for (i, mut raw) in chunk.iter().cloned().enumerate() {
                    if let Some(v) = by_i.get(&i) {
                        let effect = talaria_quality::overlay_effect(v);
                        if effect.drop {
                            continue;
                        }
                        if effect.strip_place {
                            raw.place_surface = None;
                            raw.lat = None;
                            raw.lon = None;
                        }
                        if let Some(et) = effect.event_type {
                            raw.event_type = et;
                        }
                    }
                    out.push(raw);
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "llm overlay chunk failed — keeping deterministic extracts");
                out.extend(chunk.iter().cloned());
            }
        }
    }
    out
}

async fn judge_chunk(
    client: &reqwest::Client,
    key: &str,
    model: &str,
    subject: &str,
    occupations: &str,
    items: &[talaria_quality::OverlayItem],
) -> anyhow::Result<Vec<talaria_quality::OverlayVerdict>> {
    let payload = serde_json::to_string_pretty(items)?;
    let prompt = format!(
        "You are a historical fact checker for a biography map and timeline.\n\
Subject: {subject}\nOccupations: {occupations}\n\n\
For each extracted event, return a JSON array (no markdown) of objects:\n\
{{\"i\":0,\"keep_timeline\":true,\"keep_map\":true,\"place_ok\":true,\"event_type\":null,\"reason\":\"short\"}}\n\
Rules:\n\
- keep_timeline only if the clause is about THIS person's life, not a third party.\n\
- keep_map and place_ok only if `place` is a real geographic location where THIS person was.\n\
- People, demonyms, abstract nouns, book titles, and meeting titles are not places.\n\
- battle only if this person fought or commanded there.\n\
- marriage only if this person married; office only if they held that office at that date.\n\
- event_type: corrected type or null to keep. Allowed: birth, death, residence, arrival, departure, meeting, exile, battle, siege, education, office, marriage, divorce, travel, imprisonment, diplomatic, employment, publication, historical_fact.\n\
- Do not invent events, years, or coordinates. If unsure: keep_timeline true, keep_map false.\n\n\
Events:\n{payload}"
    );

    let response = client
        .post(OPENAI_RESPONSES_URL)
        .bearer_auth(key)
        .json(&json!({
            "model": model,
            "input": prompt,
            "store": false,
        }))
        .send()
        .await?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(json!({}));
    if !status.is_success() {
        let message = body
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or(status.as_str())
            .to_string();
        anyhow::bail!(message);
    }
    let text = output_text(&body).ok_or_else(|| anyhow::anyhow!("empty llm overlay output"))?;
    talaria_quality::parse_overlay_verdicts(&text).map_err(|e| anyhow::anyhow!(e))
}

fn output_text(body: &Value) -> Option<String> {
    if let Some(s) = body.get("output_text").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    let output = body.get("output")?.as_array()?;
    for item in output {
        if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
            for part in content {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
    }
    None
}

fn first_year(surface: &str) -> Option<i32> {
    let mut digits = String::new();
    for c in surface.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            if digits.len() == 4 {
                return digits.parse().ok();
            }
        } else if !digits.is_empty() {
            digits.clear();
        }
    }
    None
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
