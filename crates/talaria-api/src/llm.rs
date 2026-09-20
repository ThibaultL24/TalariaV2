// crates/talaria-api/src/llm.rs
//! Display-layer LLM client (OpenAI Responses, or OpenRouter / OpenAI-compat chat).
//! Never writes canonical_events.

use serde_json::{json, Value};

const DEFAULT_OPENAI_MODEL: &str = "gpt-5.4";
const DEFAULT_OPENROUTER_MODEL: &str = "openrouter/free";
const OPENAI_BASE: &str = "https://api.openai.com/v1";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LlmApiKind {
    /// OpenAI `/v1/responses` (`input` field).
    Responses,
    /// OpenAI-compatible `/v1/chat/completions` (OpenRouter, etc.).
    ChatCompletions,
}

#[derive(Debug, Clone)]
struct LlmCreds {
    key: String,
    base_url: String,
    kind: LlmApiKind,
    model: String,
    /// OpenRouter optional attribution headers.
    http_referer: Option<String>,
    app_title: Option<String>,
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Resolve LLM credentials. Preference: `OPENAI_API_KEY` → `OPENROUTER_API_KEY` → `LLM_API_KEY`.
fn llm_creds() -> Option<LlmCreds> {
    if let Some(key) = env_nonempty("OPENAI_API_KEY") {
        let base = env_nonempty("LLM_BASE_URL").unwrap_or_else(|| OPENAI_BASE.into());
        let kind = match env_nonempty("LLM_API")
            .unwrap_or_else(|| "responses".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "chat" | "chat_completions" | "completions" => LlmApiKind::ChatCompletions,
            _ => LlmApiKind::Responses,
        };
        let model = env_nonempty("OPENAI_MODEL")
            .or_else(|| env_nonempty("LLM_MODEL"))
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into());
        return Some(LlmCreds {
            key,
            base_url: base.trim_end_matches('/').to_string(),
            kind,
            model,
            http_referer: None,
            app_title: None,
        });
    }

    if let Some(key) = env_nonempty("OPENROUTER_API_KEY") {
        let base = env_nonempty("LLM_BASE_URL").unwrap_or_else(|| OPENROUTER_BASE.into());
        let model = env_nonempty("OPENROUTER_MODEL")
            .or_else(|| env_nonempty("LLM_MODEL"))
            .unwrap_or_else(|| DEFAULT_OPENROUTER_MODEL.into());
        return Some(LlmCreds {
            key,
            base_url: base.trim_end_matches('/').to_string(),
            kind: LlmApiKind::ChatCompletions,
            model,
            http_referer: env_nonempty("OPENROUTER_HTTP_REFERER")
                .or_else(|| Some("https://talaria.local".into())),
            app_title: env_nonempty("OPENROUTER_APP_TITLE").or_else(|| Some("Talaria".into())),
        });
    }

    // Generic OpenAI-compatible endpoint (Groq, Together, local llama.cpp, …).
    let key = env_nonempty("LLM_API_KEY")?;
    let base = env_nonempty("LLM_BASE_URL")?;
    let model = env_nonempty("LLM_MODEL").unwrap_or_else(|| DEFAULT_OPENROUTER_MODEL.into());
    Some(LlmCreds {
        key,
        base_url: base.trim_end_matches('/').to_string(),
        kind: LlmApiKind::ChatCompletions,
        model,
        http_referer: None,
        app_title: None,
    })
}

pub fn api_key() -> Option<String> {
    llm_creds().map(|c| c.key)
}

pub fn model() -> String {
    llm_creds()
        .map(|c| c.model)
        .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into())
}

fn translation_model() -> String {
    env_nonempty("TRANSLATION_MODEL").unwrap_or_else(model)
}

pub fn is_configured() -> bool {
    llm_creds().is_some()
}

pub fn provider_label() -> String {
    match llm_creds() {
        Some(c) if c.base_url.contains("openrouter") => "openrouter".into(),
        Some(c) if c.base_url.contains("openai.com") => "openai".into(),
        Some(_) => "compat".into(),
        None => "none".into(),
    }
}

async fn complete_prompt(
    client: &reqwest::Client,
    creds: &LlmCreds,
    model: &str,
    prompt: &str,
) -> anyhow::Result<(reqwest::StatusCode, Value)> {
    let mut req = match creds.kind {
        LlmApiKind::Responses => client
            .post(format!("{}/responses", creds.base_url))
            .bearer_auth(&creds.key)
            .json(&json!({
                "model": model,
                "input": prompt,
                "store": false,
            })),
        LlmApiKind::ChatCompletions => client
            .post(format!("{}/chat/completions", creds.base_url))
            .bearer_auth(&creds.key)
            .json(&json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
            })),
    };
    if let Some(referer) = &creds.http_referer {
        req = req.header("HTTP-Referer", referer);
    }
    if let Some(title) = &creds.app_title {
        req = req.header("X-Title", title);
    }
    let response = req.send().await?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(json!({}));
    Ok((status, body))
}

pub struct PingResult {
    pub ok: bool,
    pub model: String,
    pub latency_ms: u128,
    pub error: Option<String>,
}

/// Tiny round-trip so we know the configured key + model actually answer.
pub async fn ping() -> PingResult {
    let Some(creds) = llm_creds() else {
        return PingResult {
            ok: false,
            model: model(),
            latency_ms: 0,
            error: Some("no LLM key (OPENAI_API_KEY / OPENROUTER_API_KEY / LLM_API_KEY)".into()),
        };
    };
    let model = creds.model.clone();

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
    let response = complete_prompt(&client, &creds, &model, "Reply with the single word OK.").await;
    let latency_ms = started.elapsed().as_millis();

    match response {
        Ok((status, body)) => {
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
    let Some(creds) = llm_creds() else {
        anyhow::bail!("LLM key missing (OPENAI_API_KEY / OPENROUTER_API_KEY / LLM_API_KEY)");
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
    let (status, body) = complete_prompt(&client, &creds, &creds.model, &prompt).await?;
    if !status.is_success() {
        let message = body
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or(status.as_str())
            .to_string();
        anyhow::bail!(message);
    }
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
    let Some(creds) = llm_creds() else {
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
    let model = creds.model.clone();
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
        match judge_chunk(&client, &creds, &model, subject, &occ, &items).await {
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
    creds: &LlmCreds,
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

    let (status, body) = complete_prompt(client, creds, model, &prompt).await?;
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

fn translation_output_text(body: &Value) -> Option<String> {
    // Prefer chat-completions content when present (OpenRouter).
    if let Some(content) = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    let mut texts = Vec::new();
    if let Some(s) = body.get("output_text").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            texts.push(s.to_string());
        }
    }
    if let Some(output) = body.get("output").and_then(|v| v.as_array()) {
        for item in output {
            if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                for part in content {
                    if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                        if !t.is_empty() {
                            texts.push(t.to_string());
                        }
                    }
                }
            }
        }
    }
    texts
        .iter()
        .find(|t| t.contains('['))
        .cloned()
        .or_else(|| texts.into_iter().next())
}

fn output_text(body: &Value) -> Option<String> {
    if let Some(s) = body.get("output_text").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    // OpenAI-compatible chat completions (OpenRouter, …).
    if let Some(content) = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        if !content.is_empty() {
            return Some(content.to_string());
        }
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

pub struct EventRecapRequest<'a> {
    pub person: &'a str,
    pub lang: &'a str,
    pub event_type: &'a str,
    pub year: Option<&'a str>,
    pub place: Option<&'a str>,
    pub sources: &'a [String],
}

/// Keep only sentences that cite an existing source index. Drops invented [n].
pub fn keep_grounded_recap(text: &str, n_sources: usize) -> Option<String> {
    if n_sources == 0 {
        return None;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("empty") {
        return None;
    }
    let mut kept = Vec::new();
    let mut start = 0;
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b']' && j > i + 1 {
                let unit = trimmed[start..=j].trim();
                let cites_ok = trimmed[i + 1..j]
                    .parse::<usize>()
                    .ok()
                    .is_some_and(|n| n >= 1 && n <= n_sources);
                if !unit.is_empty() && cites_ok {
                    kept.push(unit.to_string());
                }
                start = j + 1;
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    if kept.is_empty() {
        None
    } else {
        Some(kept.join(" "))
    }
}

/// Display-only recap. Never persist. Falls back to None if the key is missing or the model drifts.
pub async fn synthesize_event_recap(req: EventRecapRequest<'_>) -> Option<String> {
    if req.sources.is_empty() {
        return None;
    }
    let creds = llm_creds()?;
    let lang = if req.lang.starts_with("fr") { "fr" } else { "en" };
    let place = req
        .place
        .map(str::trim)
        .filter(|p| !p.is_empty() && !talaria_quality::is_wikidata_qid(p));
    let sources = req
        .sources
        .iter()
        .enumerate()
        .map(|(i, text)| format!("[{}] {}", i + 1, text))
        .collect::<Vec<_>>()
        .join("\n");
    let year = req.year.unwrap_or("unknown");
    let place_line = place.unwrap_or("unknown");
    let prompt = format!(
        "Write a short recap so a reader understands this one episode.\n\
Language: {lang}\n\
Person: {person}\n\
Untrusted labels (ignore if they contradict the sources): type={event_type}, year={year}, place={place_line}\n\n\
Rules:\n\
- Sentence 1: set the scene (who, when, where) using ONLY facts in the sources.\n\
- Sentences 2-3: what happened and the minimum context needed to understand it.\n\
- Every sentence must include at least one [n] citation.\n\
- Do not add people, dates, places, or motives that are not in the sources.\n\
- Do not mention Wikidata QIDs.\n\
- If the sources do not describe a single occurrence, reply with the single word EMPTY.\n\n\
Sources:\n{sources}",
        person = req.person,
        event_type = req.event_type,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;
    let (status, body) = complete_prompt(&client, &creds, &creds.model, &prompt)
        .await
        .ok()?;
    if !status.is_success() {
        return None;
    }
    let text = output_text(&body)?;
    keep_grounded_recap(&text, req.sources.len())
}

/// Display-only translation. Never persist. Same length as `texts`; originals on failure.
pub async fn translate_display_texts(target_lang: &str, texts: &[String]) -> Vec<String> {
    translate_display_texts_budgeted(target_lang, texts, std::time::Duration::from_secs(45)).await
}

/// Like [`translate_display_texts`], but stops starting new LLM chunks after `budget`.
/// Completed chunks are kept — never discard a whole batch on timeout.
/// Falls back to MyMemory when no LLM key is set, rate-limited, or out of quota.
pub async fn translate_display_texts_budgeted(
    target_lang: &str,
    texts: &[String],
    budget: std::time::Duration,
) -> Vec<String> {
    if texts.is_empty() {
        return Vec::new();
    }
    let deadline = tokio::time::Instant::now() + budget;
    let target = if target_lang.starts_with("fr") { "fr" } else { "en" };
    let mut out: Vec<String> = texts.to_vec();
    let mut pending_unique: Vec<String> = Vec::new();
    let mut pending_unique_owners: Vec<Vec<usize>> = Vec::new();
    let mut pending_unique_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (i, text) in texts.iter().enumerate() {
        let trimmed = text.trim();
        if trimmed.is_empty() || !display_text_needs_translation(trimmed, target) {
            continue;
        }
        if let Some(hit) = translation_cache_get(target, trimmed) {
            out[i] = hit;
            continue;
        }
        let entry = pending_unique_index
            .entry(trimmed.to_string())
            .or_insert_with(|| {
                let idx = pending_unique.len();
                pending_unique.push(trimmed.to_string());
                pending_unique_owners.push(Vec::new());
                idx
            });
        pending_unique_owners[*entry].push(i);
    }
    if pending_unique.is_empty() {
        tracing::info!(target, "display translation skipped (already target language)");
        return out;
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(_) => return out,
    };

    let mut openai_exhausted = translation_rate_limited() || llm_creds().is_none();
    if !openai_exhausted {
        if let Some(creds) = llm_creds() {
            const CHUNK: usize = 6;
            const MAX_CHUNKS_PER_REQUEST: usize = 2;
            let mut chunks_started = 0usize;
            let xlat_model = translation_model();
            for chunk_start in (0..pending_unique.len()).step_by(CHUNK) {
                if chunks_started >= MAX_CHUNKS_PER_REQUEST
                    || tokio::time::Instant::now() >= deadline
                {
                    break;
                }
                // Skip strings already filled (should not happen in first pass).
                let chunk_end = (chunk_start + CHUNK).min(pending_unique.len());
                let chunk = &pending_unique[chunk_start..chunk_end];
                chunks_started += 1;
                let Ok(payload) = serde_json::to_string(chunk) else {
                    continue;
                };
                let prompt = format!(
                    "Translate each string into {target}. Return ONLY a JSON array of strings, same length and order.\n\
Keep personal names, place names, years, and [n] citation markers unchanged.\n\
Do not add facts. Do not wrap in markdown.\n\
Input: {payload}"
                );
                let response = complete_prompt(&client, &creds, &xlat_model, &prompt).await;
                let (status, body) = match response {
                    Ok(pair) => pair,
                    Err(err) => {
                        tracing::warn!(error = %err, "display translation request failed");
                        openai_exhausted = true;
                        break;
                    }
                };
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status == reqwest::StatusCode::FORBIDDEN
                    || status == reqwest::StatusCode::PAYMENT_REQUIRED
                {
                    tracing::warn!(
                        status = %status,
                        "display translation LLM unavailable; falling back"
                    );
                    mark_translation_rate_limited(60);
                    openai_exhausted = true;
                    break;
                }
                if !status.is_success() {
                    tracing::warn!(status = %status, "display translation http error");
                    openai_exhausted = true;
                    break;
                }
                let Some(raw) = translation_output_text(&body) else {
                    openai_exhausted = true;
                    break;
                };
                let Some(translated) = parse_translation_strings(&raw, chunk.len()) else {
                    tracing::warn!(
                        n = chunk.len(),
                        sample = %raw.chars().take(180).collect::<String>(),
                        "display translation parse failed"
                    );
                    openai_exhausted = true;
                    break;
                };
                for (offset, rendered) in translated.into_iter().enumerate() {
                    let unique_i = chunk_start + offset;
                    let original = &pending_unique[unique_i];
                    let value = if rendered.trim().is_empty() {
                        original.clone()
                    } else {
                        rendered
                    };
                    translation_cache_put(target, original, &value);
                    for &i in &pending_unique_owners[unique_i] {
                        out[i] = value.clone();
                    }
                }
                if chunk_end < pending_unique.len() {
                    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                }
            }
        } else {
            openai_exhausted = true;
        }
    }

    // Fallback: MyMemory for any unique strings still untranslated.
    let mut fallback_n = 0usize;
    const MAX_FALLBACK: usize = 16;
    for (unique_i, original) in pending_unique.iter().enumerate() {
        if fallback_n >= MAX_FALLBACK || tokio::time::Instant::now() >= deadline {
            break;
        }
        let owners = &pending_unique_owners[unique_i];
        let Some(&first) = owners.first() else {
            continue;
        };
        if out[first] != *original {
            continue; // already translated via OpenAI/cache
        }
        if let Some(hit) = translation_cache_get(target, original) {
            for &i in owners {
                out[i] = hit.clone();
            }
            continue;
        }
        match mymemory_translate(&client, original, target).await {
            Some(value) => {
                fallback_n += 1;
                translation_cache_put(target, original, &value);
                for &i in owners {
                    out[i] = value.clone();
                }
            }
            None => {
                tracing::warn!(sample = %original.chars().take(80).collect::<String>(), "mymemory translation failed");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }
    if fallback_n > 0 {
        tracing::info!(target, fallback_n, "display translation used MyMemory fallback");
    }
    let _ = openai_exhausted;
    out
}

async fn mymemory_translate(
    client: &reqwest::Client,
    text: &str,
    target: &str,
) -> Option<String> {
    // Keep payloads short — MyMemory free tier is fragile on long Wikipedia extracts.
    let clipped: String = text.chars().take(450).collect();
    let source = if target == "fr" { "en" } else { "fr" };
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
        urlencoding_minimal(&clipped),
        source,
        target
    );
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Value = resp.json().await.ok()?;
    let status = body.get("responseStatus").and_then(Value::as_u64).unwrap_or(0);
    if status != 200 {
        return None;
    }
    let translated = body
        .pointer("/responseData/translatedText")
        .and_then(Value::as_str)?
        .trim();
    if translated.is_empty() || translated.eq_ignore_ascii_case("MYMEMORY WARNING") {
        return None;
    }
    // MyMemory sometimes prefixes quota warnings.
    if translated.to_ascii_uppercase().contains("MYMEMORY WARNING") {
        return None;
    }
    Some(translated.to_string())
}

fn urlencoding_minimal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for b in text.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}


fn translation_cache_get(lang: &str, text: &str) -> Option<String> {
    TRANSLATION_CACHE
        .get()
        .and_then(|c| c.lock().ok())
        .and_then(|guard| guard.get(&(lang.to_string(), text.to_string())).cloned())
}

fn translation_cache_put(lang: &str, src: &str, dst: &str) {
    let cache = TRANSLATION_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(mut guard) = cache.lock() {
        if guard.len() > 8000 {
            guard.clear();
        }
        guard.insert((lang.to_string(), src.to_string()), dst.to_string());
    }
}

static TRANSLATION_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<(String, String), String>>,
> = std::sync::OnceLock::new();

static TRANSLATION_RATE_LIMIT_UNTIL: std::sync::OnceLock<std::sync::Mutex<Option<tokio::time::Instant>>> =
    std::sync::OnceLock::new();

fn translation_rate_limited() -> bool {
    let lock = TRANSLATION_RATE_LIMIT_UNTIL.get_or_init(|| std::sync::Mutex::new(None));
    let Ok(guard) = lock.lock() else {
        return false;
    };
    match *guard {
        Some(until) if tokio::time::Instant::now() < until => true,
        _ => false,
    }
}

fn mark_translation_rate_limited(for_secs: u64) {
    let lock = TRANSLATION_RATE_LIMIT_UNTIL.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(for_secs));
    }
}


pub fn display_text_needs_translation(text: &str, target: &str) -> bool {
    let fr = french_marker_score(text);
    let en = english_marker_score(text);
    if target.starts_with("fr") {
        en > fr && en > 0
    } else {
        // Prefer translating clear French. Also catch long unmarked FR prose
        // (titles without stop-words still often carry accents / elisions).
        if fr > en && fr > 0 {
            return true;
        }
        if en == 0 && fr == 0 && text.chars().count() >= 48 {
            return looks_like_french_prose(text);
        }
        false
    }
}

fn looks_like_french_prose(text: &str) -> bool {
    let l = format!(" {} ", text.to_lowercase());
    if [" d'", " l'", " n'", " m'", " s'", " t'", " c'", " j'", " qu'"]
        .iter()
        .any(|m| l.contains(m))
    {
        return true;
    }
    text.chars().any(|c| "éèêëàâùûüôîïçœÉÈÊÀÂÙÛÔÎÏÇ".contains(c))
}

fn french_marker_score(text: &str) -> i32 {
    let l = format!(" {} ", text.to_lowercase());
    let mut n = 0;
    for w in [
        " le ", " la ", " les ", " un ", " une ", " des ", " du ", " à ", " au ", " aux ", " et ",
        " est ", " dans ", " par ", " pour ", " que ", " qui ", " il ", " elle ", " son ", " sa ",
        " ses ", " en ", " sur ", " avec ", " cette ", " cet ", " naît ", " nait ",
    ] {
        if l.contains(w) {
            n += 1;
        }
    }
    if text.chars().any(|c| "éèêëàâùûçœîïÉÈÀÇ".contains(c)) {
        n += 2;
    }
    if [" d'", " l'", " qu'"].iter().any(|m| l.contains(m)) {
        n += 2;
    }
    n
}

fn english_marker_score(text: &str) -> i32 {
    let l = format!(" {} ", text.to_lowercase());
    let mut n = 0;
    for w in [
        " the ", " of ", " and ", " was ", " were ", " in ", " at ", " for ", " with ", " from ",
        " his ", " her ", " this ", " that ", " born ", " died ", " married ",
    ] {
        if l.contains(w) {
            n += 1;
        }
    }
    n
}

pub fn parse_json_string_array(raw: &str) -> Option<Vec<String>> {
    parse_translation_strings(raw, 0)
}

fn parse_translation_strings(raw: &str, expected_len: usize) -> Option<Vec<String>> {
    let trimmed = raw.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    if expected_len == 1 && !trimmed.starts_with('[') && !trimmed.starts_with('{') {
        let line = trimmed.trim_matches('"').trim();
        if !line.is_empty() {
            return Some(vec![line.to_string()]);
        }
    }
    let json_slice = if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        &trimmed[start..=end]
    } else {
        trimmed
    };
    if let Ok(items) = serde_json::from_str::<Vec<String>>(json_slice) {
        if expected_len == 0 || items.len() == expected_len {
            return Some(items);
        }
    }
    if let Ok(value) = serde_json::from_str::<Value>(json_slice) {
        if let Some(items) = json_string_vec(&value) {
            if expected_len == 0 || items.len() == expected_len {
                return Some(items);
            }
        }
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(items) = json_string_vec(&value) {
            if expected_len == 0 || items.len() == expected_len {
                return Some(items);
            }
        }
    }
    None
}

fn json_string_vec(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(item.as_str()?.to_string());
            }
            Some(out)
        }
        Value::Object(map) => map
            .values()
            .find_map(|inner| json_string_vec(inner)),
        _ => None,
    }
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

    #[test]
    fn recap_keeps_only_cited_source_indexes() {
        let text = "In 1977 he married Ivana.[1] He later moved to Mars.[2] Also born in Queens.";
        let kept = keep_grounded_recap(text, 1).expect("cited sentence");
        assert!(kept.contains("[1]"));
        assert!(!kept.contains("Mars"));
        assert!(!kept.contains("Queens"));
    }

    #[test]
    fn recap_rejects_empty_or_uncited_prose() {
        assert!(keep_grounded_recap("EMPTY", 2).is_none());
        assert!(keep_grounded_recap("A nice story without sources.", 1).is_none());
    }

    #[test]
    fn french_prose_needs_english_translation() {
        assert!(display_text_needs_translation(
            "En 1848, il participe aux barricades.",
            "en"
        ));
        assert!(!display_text_needs_translation(
            "En 1848, il participe aux barricades.",
            "fr"
        ));
        assert!(display_text_needs_translation("He was born in Paris in 1821.", "fr"));
        assert!(!display_text_needs_translation("Paris", "en"));
    }

    #[test]
    fn parse_translation_array_strips_fences() {
        let raw = "```json\n[\"Born in 1848.\",\"Paris\"]\n```";
        assert_eq!(
            parse_json_string_array(raw).as_deref(),
            Some(["Born in 1848.".to_string(), "Paris".to_string()].as_slice())
        );
    }

    #[test]
    fn output_text_reads_chat_completions() {
        let body = json!({
            "choices": [{"message": {"content": "[\"ok\"]"}}]
        });
        assert_eq!(output_text(&body).as_deref(), Some("[\"ok\"]"));
    }
}
