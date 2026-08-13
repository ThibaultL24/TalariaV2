// crates/talaria-sources/src/connectors/net.rs
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::connector::ConnectorError;

pub const USER_AGENT: &str = "TalariaEngine/0.1 (+corpus; heritage connectors)";

pub fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn load_search_details(
    dir: &Path,
    id_from: fn(&serde_json::Value) -> Option<String>,
) -> Result<(serde_json::Value, HashMap<String, serde_json::Value>), ConnectorError> {
    let search_raw = std::fs::read_to_string(dir.join("search.json"))
        .map_err(|e| ConnectorError::Other(anyhow::anyhow!("fixture search: {e}")))?;
    let search: serde_json::Value = serde_json::from_str(&search_raw)
        .map_err(|e| ConnectorError::Parse(format!("fixture search: {e}")))?;

    let mut details = HashMap::new();
    let details_dir = dir.join("details");
    if details_dir.is_dir() {
        for entry in std::fs::read_dir(&details_dir)
            .map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?
        {
            let entry = entry.map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?;
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| ConnectorError::Parse(e.to_string()))?;
            let id = id_from(&value)
                .or_else(|| value.get("metadata").and_then(id_from))
                .or_else(|| value.get("object").and_then(id_from))
                .ok_or_else(|| {
                    ConnectorError::Parse(format!("missing id in {}", path.display()))
                })?;
            details.insert(id, value);
        }
    }
    Ok((search, details))
}

pub async fn get_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, ConnectorError> {
    let text = get_text(client, url).await?;
    serde_json::from_str(&text).map_err(|e| ConnectorError::Parse(e.to_string()))
}

pub async fn get_text(client: &reqwest::Client, url: &str) -> Result<String, ConnectorError> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 429 {
            if attempt >= 3 {
                return Err(ConnectorError::RateLimited);
            }
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(2u64.saturating_pow(attempt));
            tokio::time::sleep(Duration::from_secs(wait.min(30))).await;
            continue;
        }
        if status.is_server_error() && attempt < 3 {
            tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
            continue;
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {body}")));
        }
        return resp
            .text()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()));
    }
}

pub fn first_str(v: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        match v.get(*key) {
            Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                return Some(s.trim().to_string());
            }
            Some(serde_json::Value::Array(arr)) => {
                if let Some(s) = arr.iter().filter_map(|x| x.as_str()).find(|s| !s.trim().is_empty())
                {
                    return Some(s.trim().to_string());
                }
            }
            Some(serde_json::Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

pub fn year_from(surface: &str) -> Option<i32> {
    let digits: String = surface.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}

pub fn build_client(timeout: Duration) -> Result<reqwest::Client, ConnectorError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .build()
        .map_err(|e| ConnectorError::Http(e.to_string()))
}
