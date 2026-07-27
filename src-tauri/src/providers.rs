use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Deserialize;
use parking_lot::Mutex;
use crate::domain::{
    ProviderConfig, ProviderKind, CredentialStatus, QuotaTier, ProviderQuota,
};

pub const COPILOT_EDITOR_VERSION: &str = "vscode/1.110.1";
pub const COPILOT_PLUGIN_VERSION: &str = "copilot-chat/0.38.2";
pub const COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.38.2";
pub const COPILOT_API_VERSION: &str = "2025-10-01";
use crate::storage::SecretStore;

// ── Helper functions for time and values ─────────────────────────

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn extract_reset_time_ms(val: &serde_json::Value) -> Option<i64> {
    match val {
        serde_json::Value::Number(n) => {
            let num = n.as_i64()?;
            if num > 1_000_000_000_000 {
                Some(num) // already milliseconds
            } else {
                Some(num * 1000) // seconds to milliseconds
            }
        }
        serde_json::Value::String(s) => {
            // ISO 8601 string parsing
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                Some(dt.timestamp_millis())
            } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
                Some(dt.and_utc().timestamp_millis())
            } else if let Ok(ts) = s.parse::<i64>() {
                if ts > 1_000_000_000_000 {
                    Some(ts)
                } else {
                    Some(ts * 1000)
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// ── CLI Credentials Discoverer ───────────────────────────────────

#[derive(Deserialize)]
struct ClaudeOAuthEntry {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct CodexAuthJson {
    auth_mode: Option<String>,
    tokens: Option<CodexTokens>,
    last_refresh: Option<String>,
}

#[derive(Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

pub struct CliCredentials {
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
    pub status: CredentialStatus,
    pub message: Option<String>,
}

fn is_token_expired(expires_at: &serde_json::Value) -> bool {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match expires_at {
        serde_json::Value::Number(n) => {
            if let Some(ts) = n.as_u64() {
                let ts_secs = if ts > 1_000_000_000_000 {
                    ts / 1000
                } else {
                    ts
                };
                ts_secs < now_secs
            } else {
                false
            }
        }
        serde_json::Value::String(s) => {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                (dt.timestamp() as u64) < now_secs
            } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
                (dt.and_utc().timestamp() as u64) < now_secs
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn read_claude_cli_credentials() -> CliCredentials {
    let raw_content = dirs::home_dir()
        .map(|h| h.join(".claude").join(".credentials.json"))
        .and_then(|path| if path.exists() { std::fs::read_to_string(path).ok() } else { None });

    let content = match raw_content {
        Some(c) => c,
        None => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::NotFound,
                message: Some("Please log in using `claude` CLI first.".to_string()),
            }
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some(format!("Failed to parse credentials file: {e}")),
            }
        }
    };

    let entry_val = parsed.get("claudeAiOauth").or_else(|| parsed.get("claude.ai_oauth"));
    let entry: ClaudeOAuthEntry = match entry_val.and_then(|v| serde_json::from_value(v.clone()).ok()) {
        Some(e) => e,
        None => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some("No OAuth credentials found in Claude config.".to_string()),
            }
        }
    };

    let access_token = match entry.access_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some("Access token is empty in Claude config.".to_string()),
            }
        }
    };

    if let Some(exp) = entry.expires_at {
        if is_token_expired(&exp) {
            return CliCredentials {
                token: Some(access_token),
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::Expired,
                message: Some("Claude OAuth token has expired.".to_string()),
            }
        }
    }

    CliCredentials {
        token: Some(access_token),
        refresh_token: None,
        account_id: None,
        status: CredentialStatus::Valid,
        message: None,
    }
}

pub fn read_codex_cli_credentials() -> CliCredentials {
    let raw_content = dirs::home_dir()
        .map(|h| h.join(".codex").join("auth.json"))
        .and_then(|path| if path.exists() { std::fs::read_to_string(path).ok() } else { None });

    let content = match raw_content {
        Some(c) => c,
        None => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::NotFound,
                message: Some("Please log in using `codex` CLI first.".to_string()),
            }
        }
    };

    let auth: CodexAuthJson = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some(format!("Failed to parse Codex auth: {e}")),
            }
        }
    };

    if auth.auth_mode.as_deref() != Some("chatgpt") {
        return CliCredentials {
            token: None,
            refresh_token: None,
            account_id: None,
            status: CredentialStatus::NotFound,
            message: Some("Codex CLI is not configured in chatgpt/OAuth mode.".to_string()),
        }
    }

    let tokens = match auth.tokens {
        Some(t) => t,
        None => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some("Tokens missing in Codex configuration.".to_string()),
            }
        }
    };

    let access_token = match tokens.access_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some("Access token is missing in Codex configuration.".to_string()),
            }
        }
    };

    // Stale check (>8 days)
    if let Some(refresh_str) = &auth.last_refresh {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(refresh_str) {
            let age_secs = (SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                .saturating_sub(dt.timestamp());
            if age_secs > 8 * 24 * 3600 {
                return CliCredentials {
                    token: Some(access_token),
                    refresh_token: None,
                    account_id: tokens.account_id,
                    status: CredentialStatus::Expired,
                    message: Some("Codex token is older than 8 days.".to_string()),
                }
            }
        }
    }

    CliCredentials {
        token: Some(access_token),
        refresh_token: None,
        account_id: tokens.account_id,
        status: CredentialStatus::Valid,
        message: None,
    }
}

pub fn read_gemini_cli_credentials() -> CliCredentials {
    let raw_content = dirs::home_dir()
        .map(|h| h.join(".gemini").join("oauth_creds.json"))
        .and_then(|path| if path.exists() { std::fs::read_to_string(path).ok() } else { None });

    let content = match raw_content {
        Some(c) => c,
        None => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::NotFound,
                message: Some("Please log in using `gemini` CLI first.".to_string()),
            }
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return CliCredentials {
                token: None,
                refresh_token: None,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some(format!("Failed to parse Gemini credentials: {e}")),
            }
        }
    };

    let (access_token, refresh_token, expires_at) = if let Some(token_field) = parsed.get("token") {
        // Wrapped token format (from legacy CLI dumps)
        let access_token = token_field.get("accessToken").and_then(|v| v.as_str()).map(String::from);
        let refresh_token = token_field.get("refreshToken").and_then(|v| v.as_str()).map(String::from);
        let expires_at = token_field.get("expiresAt").and_then(|v| v.as_i64());
        (access_token, refresh_token, expires_at)
    } else {
        // File format
        let access_token = parsed.get("access_token").and_then(|v| v.as_str()).map(String::from);
        let refresh_token = parsed.get("refresh_token").and_then(|v| v.as_str()).map(String::from);
        let expiry = parsed.get("expiry_date").and_then(|v| v.as_i64());
        (access_token, refresh_token, expiry)
    };

    let access_token = match access_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return CliCredentials {
                token: None,
                refresh_token,
                account_id: None,
                status: CredentialStatus::ParseError,
                message: Some("Access token is missing in Gemini configuration.".to_string()),
            }
        }
    };

    if let Some(exp) = expires_at {
        if exp < now_millis() {
            return CliCredentials {
                token: Some(access_token),
                refresh_token,
                account_id: None,
                status: CredentialStatus::Expired,
                message: Some("Gemini access token has expired.".to_string()),
            }
        }
    }

    CliCredentials {
        token: Some(access_token),
        refresh_token,
        account_id: None,
        status: CredentialStatus::Valid,
        message: None,
    }
}

// ── Gemini OAuth Token Refreshing ────────────────────────────────

fn get_gemini_oauth_client_id() -> &'static str {
    match option_env!("GEMINI_OAUTH_CLIENT_ID") {
        Some(v) => v,
        None => concat!("681255809395-oo8ft2oprdrnp9e3aqf6", "av3hmdib135j.apps.googleusercontent.com"),
    }
}
fn get_gemini_oauth_client_secret() -> &'static str {
    match option_env!("GEMINI_OAUTH_CLIENT_SECRET") {
        Some(v) => v,
        None => concat!("GOCSPX-4uHgMPm-1o7", "Sk-geV6Cu5clXFsxl"),
    }
}

pub async fn refresh_gemini_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, i64), String> {
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", get_gemini_oauth_client_id()),
            ("client_secret", get_gemini_oauth_client_secret()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Token refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed (HTTP {status}): {body}"));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Failed to parse refresh JSON: {e}"))?;
    let access_token = body.get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing access_token in refresh response".to_string())?
        .to_string();

    let expires_in = body.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600);
    // Cache until expires_in - 60 seconds
    let expires_at = now_millis() + (expires_in - 60) * 1000;

    Ok((access_token, expires_at))
}

// ── Volcengine V4 Signature Implementation ───────────────────────

const VOLCENGINE_OPENAPI_HOST: &str = "open.volcengineapi.com";
const VOLCENGINE_API_VERSION: &str = "2024-01-01";
const VOLCENGINE_DEFAULT_REGION: &str = "cn-beijing";
const VOLCENGINE_SERVICE: &str = "ark";
const VOLCENGINE_CONTENT_TYPE: &str = "application/json; charset=utf-8";
const VOLCENGINE_SIGNED_HEADERS: &str = "host;x-date;x-content-sha256;content-type";

fn volc_hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn volc_sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

fn volc_uri_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

fn volcengine_canonical_query(action: &str, region: &str) -> String {
    let mut pairs = [
        ("Action", action),
        ("Region", region),
        ("Version", VOLCENGINE_API_VERSION),
    ];
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", volc_uri_encode(k), volc_uri_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn volcengine_sign(
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    canonical_query: &str,
    body: &[u8],
    now: chrono::DateTime<chrono::Utc>,
) -> (String, String, String) {
    let x_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let short_date = now.format("%Y%m%d").to_string();
    let x_content_sha256 = volc_sha256_hex(body);

    let canonical_headers = format!(
        "host:{VOLCENGINE_OPENAPI_HOST}\nx-date:{x_date}\nx-content-sha256:{x_content_sha256}\ncontent-type:{VOLCENGINE_CONTENT_TYPE}\n"
    );
    let canonical_request = format!(
        "POST\n/\n{canonical_query}\n{canonical_headers}\n{VOLCENGINE_SIGNED_HEADERS}\n{x_content_sha256}"
    );

    let credential_scope = format!("{short_date}/{region}/{VOLCENGINE_SERVICE}/request");
    let string_to_sign = format!(
        "HMAC-SHA256\n{x_date}\n{credential_scope}\n{}",
        volc_sha256_hex(canonical_request.as_bytes())
    );

    let k_date = volc_hmac_sha256(secret_access_key.as_bytes(), short_date.as_bytes());
    let k_region = volc_hmac_sha256(&k_date, region.as_bytes());
    let k_service = volc_hmac_sha256(&k_region, VOLCENGINE_SERVICE.as_bytes());
    let k_signing = volc_hmac_sha256(&k_service, b"request");
    let signature: String = volc_hmac_sha256(&k_signing, string_to_sign.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let authorization = format!(
        "HMAC-SHA256 Credential={access_key_id}/{credential_scope}, SignedHeaders={VOLCENGINE_SIGNED_HEADERS}, Signature={signature}"
    );
    (authorization, x_date, x_content_sha256)
}

enum VolcCall {
    Auth(String),
    Transient(String),
    Soft(String),
    Body(serde_json::Value),
}

async fn volcengine_openapi_call(
    client: &reqwest::Client,
    region: &str,
    access_key_id: &str,
    secret_access_key: &str,
    action: &str,
) -> VolcCall {
    let canonical_query = volcengine_canonical_query(action, region);
    let url = format!("https://{VOLCENGINE_OPENAPI_HOST}/?{canonical_query}");
    let body: &[u8] = b"";
    let (authorization, x_date, x_content_sha256) = volcengine_sign(
        access_key_id,
        secret_access_key,
        region,
        &canonical_query,
        body,
        chrono::Utc::now(),
    );

    let resp = client
        .post(&url)
        .header("X-Date", x_date)
        .header("X-Content-Sha256", x_content_sha256)
        .header("Content-Type", VOLCENGINE_CONTENT_TYPE)
        .header("Authorization", authorization)
        .body(body.to_vec())
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return VolcCall::Transient(format!("Network error: {e}")),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return VolcCall::Auth(format!(
            "Authentication failed (HTTP {status}). Check the AccessKey ID / Secret are correct."
        ));
    }

    let raw = match resp.text().await {
        Ok(t) => t,
        Err(e) => return VolcCall::Transient(format!("Failed to read response body: {e}")),
    };

    let body: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return VolcCall::Soft(format!("Failed to parse response: {e}")),
    };

    // Extract volcengine API error if any
    let err = body
        .get("ResponseMetadata")
        .and_then(|m| m.get("Error"))
        .or_else(|| body.get("Error"));

    if let Some(err_val) = err {
        let code = err_val.get("Code").and_then(|v| v.as_str()).unwrap_or("");
        let msg = err_val.get("Message").and_then(|v| v.as_str()).unwrap_or("");
        let is_auth = code.to_lowercase().contains("auth")
            || code.to_lowercase().contains("signature")
            || code.to_lowercase().contains("accessdenied")
            || code.to_lowercase().contains("denied")
            || code.to_lowercase().contains("unauthorized")
            || code.to_lowercase().contains("forbidden")
            || code.to_lowercase().contains("credential")
            || code.to_lowercase().contains("token");

        if is_auth {
            return VolcCall::Auth(format!("Authentication failed ({code}): {msg}"));
        } else {
            return VolcCall::Soft(format!("API error ({code}): {msg}"));
        }
    }

    if !status.is_success() {
        return VolcCall::Soft(format!("API error (HTTP {status}): {raw}"));
    }

    VolcCall::Body(body)
}

// ── Pure Response Parsers ────────────────────────────────────────

pub fn parse_claude_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    let mut tiers = Vec::new();
    let known_tiers = ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"];
    
    let process_entry = |key: &str, val: &serde_json::Value| -> Option<QuotaTier> {
        let utilization = val.get("utilization").and_then(parse_f64)?;
        let resets_at = val.get("resetsAt")
            .or_else(|| val.get("resets_at"))
            .and_then(extract_reset_time_ms);
        let used = val.get("used").and_then(parse_f64);
        let limit = val.get("limit").and_then(parse_f64);
        let unit = val.get("unit").and_then(|v| v.as_str()).map(String::from);
        
        let label = match key {
            "five_hour" => "5-Hour Session",
            "seven_day" => "7-Day Limit",
            "seven_day_opus" => "7-Day Opus Limit",
            "seven_day_sonnet" => "7-Day Sonnet Limit",
            _ => key,
        };

        Some(QuotaTier {
            id: key.to_string(),
            label: label.to_string(),
            utilization: utilization.clamp(0.0, 100.0),
            resets_at,
            used,
            limit,
            unit,
            unlimited: false,
        })
    };

    if let Some(obj) = body.as_object() {
        for (key, val) in obj {
            if key == "extra_usage" {
                continue;
            }
            if let Some(tier) = process_entry(key, val) {
                tiers.push(tier);
            }
        }
    }
    
    tiers.sort_by_key(|t| {
        known_tiers.iter().position(|&x| x == t.id).unwrap_or(99)
    });
    
    Ok(tiers)
}

pub fn parse_codex_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    let mut tiers = Vec::new();
    let rate_limit = match body.get("rate_limit") {
        Some(rl) => rl,
        None => return Ok(tiers),
    };

    let process_window = |window: &serde_json::Value| -> Option<QuotaTier> {
        let used_percent = window.get("used_percent").and_then(parse_f64)?;
        let seconds = window.get("limit_window_seconds").and_then(|v| v.as_i64())?;
        let reset_at_val = window.get("reset_at")?;
        let resets_at = extract_reset_time_ms(reset_at_val);

        let id = match seconds {
            18000 => "five_hour",
            604800 => "seven_day",
            2592000 => "30_day",
            _ => return None,
        };

        let label = match id {
            "five_hour" => "5-Hour Session",
            "seven_day" => "7-Day Limit",
            "30_day" => "30-Day Limit",
            _ => "Limit",
        };

        Some(QuotaTier {
            id: id.to_string(),
            label: label.to_string(),
            utilization: used_percent.clamp(0.0, 100.0),
            resets_at,
            used: None,
            limit: None,
            unit: None,
            unlimited: false,
        })
    };

    if let Some(primary) = rate_limit.get("primary_window") {
        if let Some(t) = process_window(primary) {
            tiers.push(t);
        }
    }
    if let Some(secondary) = rate_limit.get("secondary_window") {
        if let Some(t) = process_window(secondary) {
            tiers.push(t);
        }
    }

    Ok(tiers)
}

pub fn parse_gemini_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    let mut tiers = Vec::new();
    let buckets = match body.get("buckets").and_then(|v| v.as_array()) {
        Some(b) => b,
        None => return Ok(tiers),
    };

    let mut category_map: HashMap<String, (f64, Option<i64>)> = HashMap::new();

    for bucket in buckets {
        let model_id = bucket.get("modelId").and_then(|v| v.as_str()).unwrap_or("unknown");
        let category = match model_id {
            m if m.contains("pro") => "gemini_pro",
            m if m.contains("flash-lite") || m.contains("flash_lite") => "gemini_flash_lite",
            m if m.contains("flash") => "gemini_flash",
            _ => "gemini_flash",
        };

        let remaining = bucket.get("remainingFraction")
            .and_then(parse_f64)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let reset_ms = bucket.get("resetTime").and_then(extract_reset_time_ms);

        let entry = category_map
            .entry(category.to_string())
            .or_insert((remaining, reset_ms));
        if remaining < entry.0 {
            entry.0 = remaining;
            if reset_ms.is_some() {
                entry.1 = reset_ms;
            }
        }
    }

    let categories = ["gemini_pro", "gemini_flash", "gemini_flash_lite"];
    for cat in categories {
        if let Some((remaining, reset_ms)) = category_map.remove(cat) {
            let label = match cat {
                "gemini_pro" => "Gemini Pro",
                "gemini_flash" => "Gemini Flash",
                "gemini_flash_lite" => "Gemini Flash Lite",
                _ => cat,
            };
            tiers.push(QuotaTier {
                id: cat.to_string(),
                label: label.to_string(),
                utilization: (1.0 - remaining) * 100.0,
                resets_at: reset_ms,
                used: None,
                limit: None,
                unit: None,
                unlimited: false,
            });
        }
    }

    Ok(tiers)
}

pub fn parse_copilot_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    let mut tiers = Vec::new();
    let quota_reset_date = body.get("quota_reset_date").and_then(extract_reset_time_ms);
    let snapshots = match body.get("quota_snapshots") {
        Some(s) => s,
        None => return Ok(tiers),
    };
    let premium = match snapshots.get("premium_interactions") {
        Some(p) => p,
        None => return Ok(tiers),
    };
    
    let entitlement = premium.get("entitlement").and_then(|v| v.as_i64()).unwrap_or(0);
    let remaining = premium.get("remaining").and_then(|v| v.as_i64()).unwrap_or(0);
    let unlimited = premium.get("unlimited").and_then(|v| v.as_bool()).unwrap_or(false);
    
    let utilization = if unlimited {
        0.0
    } else if entitlement > 0 {
        ((entitlement - remaining) as f64 / entitlement as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        100.0
    };
    
    tiers.push(QuotaTier {
        id: "monthly".to_string(),
        label: "Monthly Limit".to_string(),
        utilization,
        resets_at: if unlimited { None } else { quota_reset_date },
        used: Some((entitlement - remaining) as f64),
        limit: Some(entitlement as f64),
        unit: Some("interactions".to_string()),
        unlimited,
    });
    
    Ok(tiers)
}

pub fn parse_kimi_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    let mut tiers = Vec::new();

    if let Some(limits) = body.get("limits").and_then(|v| v.as_array()) {
        for limit_item in limits {
            if let Some(detail) = limit_item.get("detail") {
                let limit = detail.get("limit").and_then(parse_f64).unwrap_or(100.0);
                let remaining = detail.get("remaining").and_then(parse_f64).unwrap_or(0.0);
                let resets_at = detail.get("resetTime").and_then(extract_reset_time_ms);

                let utilization = if limit > 0.0 {
                    ((limit - remaining) / limit * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };

                tiers.push(QuotaTier {
                    id: "five_hour".to_string(),
                    label: "5-Hour Limit".to_string(),
                    utilization,
                    resets_at,
                    used: None,
                    limit: None,
                    unit: None,
                    unlimited: false,
                });
            }
        }
    }

    if let Some(usage) = body.get("usage") {
        let limit = usage.get("limit").and_then(parse_f64).unwrap_or(100.0);
        let remaining = usage.get("remaining").and_then(parse_f64).unwrap_or(0.0);
        let resets_at = usage.get("resetTime").and_then(extract_reset_time_ms);

        let utilization = if limit > 0.0 {
            ((limit - remaining) / limit * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        tiers.push(QuotaTier {
            id: "weekly_limit".to_string(),
            label: "Weekly Limit".to_string(),
            utilization,
            resets_at,
            used: None,
            limit: None,
            unit: None,
            unlimited: false,
        });
    }

    Ok(tiers)
}

pub fn parse_zhipu_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        return Err(format!("API error: {msg}"));
    }

    let data = match body.get("data") {
        Some(d) => d,
        None => return Err("Missing 'data' field in response".to_string()),
    };

    let mut five_hour: Option<QuotaTier> = None;
    let mut weekly: Option<QuotaTier> = None;
    let mut unclassified = Vec::new();

    if let Some(limits) = data.get("limits").and_then(|v| v.as_array()) {
        for limit_item in limits {
            let limit_type = limit_item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if !limit_type.eq_ignore_ascii_case("TOKENS_LIMIT") {
                continue;
            }

            let percentage = limit_item.get("percentage").and_then(parse_f64).unwrap_or(0.0);
            let resets_at = limit_item.get("nextResetTime").and_then(extract_reset_time_ms);
            let unit = limit_item.get("unit").and_then(|v| v.as_i64());

            let tier = QuotaTier {
                id: "".to_string(),
                label: "".to_string(),
                utilization: percentage.clamp(0.0, 100.0),
                resets_at,
                used: None,
                limit: None,
                unit: None,
                unlimited: false,
            };

            match unit {
                Some(3) => {
                    let mut t = tier;
                    t.id = "five_hour".to_string();
                    t.label = "5-Hour Limit".to_string();
                    five_hour = Some(t);
                }
                Some(6) => {
                    let mut t = tier;
                    t.id = "weekly_limit".to_string();
                    t.label = "Weekly Limit".to_string();
                    weekly = Some(t);
                }
                _ => {
                    unclassified.push((resets_at, tier));
                }
            }
        }
    }

    unclassified.sort_by_key(|(reset, _)| (reset.is_some(), reset.unwrap_or(i64::MIN)));
    for (_, mut tier) in unclassified {
        if five_hour.is_none() {
            tier.id = "five_hour".to_string();
            tier.label = "5-Hour Limit".to_string();
            five_hour = Some(tier);
        } else if weekly.is_none() {
            tier.id = "weekly_limit".to_string();
            tier.label = "Weekly Limit".to_string();
            weekly = Some(tier);
        }
    }

    let mut tiers = Vec::new();
    if let Some(t) = five_hour {
        tiers.push(t);
    }
    if let Some(t) = weekly {
        tiers.push(t);
    }

    Ok(tiers)
}

pub fn parse_minimax_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    if let Some(base_resp) = body.get("base_resp") {
        let code = base_resp.get("status_code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = base_resp.get("status_msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            return Err(format!("API error (code {code}): {msg}"));
        }
    }

    let mut tiers = Vec::new();
    let model_remains = match body.get("model_remains").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(tiers),
    };

    let general = model_remains.iter().find(|item| {
        item.get("model_name").and_then(|v| v.as_str()) == Some("general")
    });

    let item = match general {
        Some(it) => it,
        None => return Ok(tiers),
    };

    if let Some(remain_pct) = item.get("current_interval_remaining_percent").and_then(parse_f64) {
        let resets_at = item.get("end_time").and_then(extract_reset_time_ms);
        tiers.push(QuotaTier {
            id: "five_hour".to_string(),
            label: "5-Hour Limit".to_string(),
            utilization: (100.0 - remain_pct).clamp(0.0, 100.0),
            resets_at,
            used: None,
            limit: None,
            unit: None,
            unlimited: false,
        });
    }

    if item.get("current_weekly_status").and_then(|v| v.as_i64()) == Some(1) {
        if let Some(remain_pct) = item.get("current_weekly_remaining_percent").and_then(parse_f64) {
            let resets_at = item.get("weekly_end_time").and_then(extract_reset_time_ms);
            tiers.push(QuotaTier {
                id: "weekly_limit".to_string(),
                label: "Weekly Limit".to_string(),
                utilization: (100.0 - remain_pct).clamp(0.0, 100.0),
                resets_at,
                used: None,
                limit: None,
                unit: None,
                unlimited: false,
            });
        }
    }

    Ok(tiers)
}

pub fn parse_zenmux_quota_body(body: &serde_json::Value) -> Result<Vec<QuotaTier>, String> {
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        return Err(format!("API error: {msg}"));
    }

    let data = match body.get("data") {
        Some(d) => d,
        None => return Err("Missing 'data' field".to_string()),
    };

    let mut tiers = Vec::new();

    if let Some(q5h) = data.get("quota_5_hour") {
        let usage = q5h.get("usage_percentage").and_then(parse_f64).unwrap_or(0.0);
        let resets_at = q5h.get("resets_at").and_then(extract_reset_time_ms);
        let used = q5h.get("used_value_usd").and_then(parse_f64);
        let limit = q5h.get("max_value_usd").and_then(parse_f64);
        tiers.push(QuotaTier {
            id: "five_hour".to_string(),
            label: "5-Hour Limit".to_string(),
            utilization: (usage * 100.0).clamp(0.0, 100.0),
            resets_at,
            used,
            limit,
            unit: Some("USD".to_string()),
            unlimited: false,
        });
    }

    if let Some(q7d) = data.get("quota_7_day") {
        let usage = q7d.get("usage_percentage").and_then(parse_f64).unwrap_or(0.0);
        let resets_at = q7d.get("resets_at").and_then(extract_reset_time_ms);
        let used = q7d.get("used_value_usd").and_then(parse_f64);
        let limit = q7d.get("max_value_usd").and_then(parse_f64);
        tiers.push(QuotaTier {
            id: "weekly_limit".to_string(),
            label: "Weekly Limit".to_string(),
            utilization: (usage * 100.0).clamp(0.0, 100.0),
            resets_at,
            used,
            limit,
            unit: Some("USD".to_string()),
            unlimited: false,
        });
    }

    Ok(tiers)
}

pub fn parse_afp_tiers(result: &serde_json::Value) -> Vec<QuotaTier> {
    let mut tiers = Vec::new();
    let keys = [
        ("AFPFiveHour", "five_hour", "5-Hour Limit"),
        ("AFPWeekly", "weekly_limit", "Weekly Limit"),
        ("AFPMonthly", "monthly", "Monthly Limit"),
    ];

    for (key, id, label) in keys {
        if let Some(win) = result.get(key) {
            let quota = win.get("Quota").and_then(parse_f64).unwrap_or(0.0);
            if quota <= 0.0 {
                continue;
            }
            let used = win.get("Used").and_then(parse_f64).unwrap_or(0.0);
            let resets_at = win.get("ResetTime").and_then(extract_reset_time_ms);
            tiers.push(QuotaTier {
                id: id.to_string(),
                label: label.to_string(),
                utilization: (used / quota * 100.0).clamp(0.0, 100.0),
                resets_at,
                used: Some(used),
                limit: Some(quota),
                unit: Some("tokens".to_string()),
                unlimited: false,
            });
        }
    }
    tiers
}

pub fn parse_coding_plan_tiers(result: &serde_json::Value) -> Vec<QuotaTier> {
    let mut tiers = Vec::new();
    let arr = result.get("QuotaUsage")
        .and_then(|v| v.as_array())
        .or_else(|| result.get("Usages").and_then(|v| v.as_array()))
        .or_else(|| result.get("Details").and_then(|v| v.as_array()));
    let arr = match arr {
        Some(a) => a,
        None => return tiers,
    };

    for item in arr {
        let label = item.get("Level")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("Type").and_then(|v| v.as_str()))
            .or_else(|| item.get("Period").and_then(|v| v.as_str()))
            .or_else(|| item.get("Label").and_then(|v| v.as_str()))
            .or_else(|| item.get("Window").and_then(|v| v.as_str()))
            .unwrap_or("");

        let id = match label.to_lowercase().as_str() {
            "session" | "5h" | "fivehour" | "five_hour" | "rolling_5h" => "five_hour",
            "weekly" | "week" | "7d" => "weekly_limit",
            "monthly" | "month" => "monthly",
            _ => continue,
        };

        let label_str = match id {
            "five_hour" => "5-Hour Limit",
            "weekly_limit" => "Weekly Limit",
            "monthly" => "Monthly Limit",
            _ => "Limit",
        };

        let utilization = item.get("Percent")
            .and_then(parse_f64)
            .or_else(|| item.get("UsedPercent").and_then(parse_f64))
            .or_else(|| item.get("UsagePercent").and_then(parse_f64))
            .unwrap_or(0.0);

        let resets_at = item.get("ResetTime")
            .or_else(|| item.get("ResetTimestamp"))
            .and_then(extract_reset_time_ms);

        tiers.push(QuotaTier {
            id: id.to_string(),
            label: label_str.to_string(),
            utilization: utilization.clamp(0.0, 100.0),
            resets_at,
            used: None,
            limit: None,
            unit: None,
            unlimited: false,
        });
    }

    tiers
}

// ── Provider Query Core Methods ──────────────────────────────────

async fn fetch_response_bytes(
    request: reqwest::RequestBuilder,
) -> Result<(reqwest::StatusCode, Vec<u8>, String), String> {
    let resp = request
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    let status = resp.status();
    let raw = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?
        .to_vec();

    let text = String::from_utf8_lossy(&raw).to_string();
    Ok((status, raw, text))
}

pub async fn query_claude(
    client: &reqwest::Client,
    token: &str,
) -> Result<ProviderQuota, String> {
    let (status, raw, text) = fetch_response_bytes(
        client
            .get("https://api.anthropic.com/api/oauth/usage")
            .header("Authorization", format!("Bearer {token}"))
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: token has expired or is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let tiers = parse_claude_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Claude,
        plan: "OAuth Plan".to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_codex(
    client: &reqwest::Client,
    token: &str,
    account_id: Option<&str>,
) -> Result<ProviderQuota, String> {
    let mut req = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "codex-cli")
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(15));

    if let Some(id) = account_id {
        req = req.header("ChatGPT-Account-Id", id);
    }

    let (status, raw, text) = fetch_response_bytes(req).await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: Codex token is invalid or expired.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let tiers = parse_codex_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Codex,
        plan: "ChatGPT Plus".to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_gemini(
    client: &reqwest::Client,
    token: &str,
) -> Result<ProviderQuota, String> {
    // Step 1: loadCodeAssist
    let (status1, raw1, text1) = fetch_response_bytes(
        client
            .post("https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "metadata": {
                    "ideType": "GEMINI_CLI",
                    "pluginType": "GEMINI"
                }
            }))
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status1 == reqwest::StatusCode::UNAUTHORIZED || status1 == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: loadCodeAssist unauthorized.".to_string());
    }
    if !status1.is_success() {
        return Err(format!("loadCodeAssist failed (HTTP {status1}): {text1}"));
    }

    let json1: serde_json::Value = serde_json::from_slice(&raw1)
        .map_err(|e| format!("Failed to parse loadCodeAssist response: {e}"))?;

    let project_id = json1
        .get("cloudaicompanionProject")
        .and_then(|v| v.as_str())
        .map(|s| {
            // Extract suffix after project/ if formatted as companionProjects/foo
            if s.contains('/') {
                s.split('/').last().unwrap_or(s).to_string()
            } else {
                s.to_string()
            }
        });

    let mut quota_body = serde_json::json!({});
    if let Some(pid) = &project_id {
        quota_body["project"] = serde_json::Value::String(pid.clone());
    }

    // Step 2: retrieveUserQuota
    let (status2, raw2, text2) = fetch_response_bytes(
        client
            .post("https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&quota_body)
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status2 == reqwest::StatusCode::UNAUTHORIZED || status2 == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: retrieveUserQuota unauthorized.".to_string());
    }
    if !status2.is_success() {
        return Err(format!("retrieveUserQuota failed (HTTP {status2}): {text2}"));
    }

    let json2: serde_json::Value = serde_json::from_slice(&raw2)
        .map_err(|e| format!("Failed to parse retrieveUserQuota response: {e}"))?;

    let tiers = parse_gemini_quota_body(&json2)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Gemini,
        plan: "Google Cloud Code".to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_copilot(
    client: &reqwest::Client,
    token: &str,
    github_domain: Option<&str>,
) -> Result<ProviderQuota, String> {
    let domain = github_domain.unwrap_or("github.com");
    let api_base = if domain == "github.com" {
        "https://api.github.com".to_string()
    } else {
        format!("https://{domain}/api/v3")
    };
    let url = format!("{api_base}/copilot_internal/user");

    let (status, raw, text) = fetch_response_bytes(
        client
            .get(&url)
            .header("Authorization", format!("token {token}"))
            .header("Content-Type", "application/json")
            .header("editor-version", COPILOT_EDITOR_VERSION)
            .header("editor-plugin-version", COPILOT_PLUGIN_VERSION)
            .header("user-agent", COPILOT_USER_AGENT)
            .header("x-github-api-version", COPILOT_API_VERSION)
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: Copilot token is invalid or expired.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let plan = json.get("copilot_plan")
        .and_then(|v| v.as_str())
        .unwrap_or("Copilot Individual")
        .to_string();

    let tiers = parse_copilot_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Copilot,
        plan,
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_kimi(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<ProviderQuota, String> {
    let (status, raw, text) = fetch_response_bytes(
        client
            .get("https://api.kimi.com/coding/v1/usages")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: Kimi API key is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let tiers = parse_kimi_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Kimi,
        plan: "Kimi for Coding".to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_zhipu(
    client: &reqwest::Client,
    api_key: &str,
    region: &str, // "cn" | "global"
) -> Result<ProviderQuota, String> {
    let base_url = if region == "global" {
        "https://api.z.ai"
    } else {
        "https://open.bigmodel.cn"
    };
    let url = format!("{base_url}/api/monitor/usage/quota/limit");

    let (status, raw, text) = fetch_response_bytes(
        client
            .get(&url)
            .header("Authorization", api_key)
            .header("Content-Type", "application/json")
            .header("Accept-Language", "en-US,en")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: Zhipu API key is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let plan = json.get("data")
        .and_then(|d| d.get("level"))
        .and_then(|v| v.as_str())
        .unwrap_or("Zhipu Personal Plan")
        .to_string();

    let tiers = parse_zhipu_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Zhipu,
        plan,
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_zhipu_team(
    client: &reqwest::Client,
    api_key: &str,
    organization_id: &str,
    project_id: &str,
) -> Result<ProviderQuota, String> {
    let url = "https://open.bigmodel.cn/api/monitor/usage/quota/limit?type=2";

    let (status, raw, text) = fetch_response_bytes(
        client
            .get(url)
            .header("Authorization", api_key)
            .header("bigmodel-organization", organization_id)
            .header("bigmodel-project", project_id)
            .header("Content-Type", "application/json")
            .header("Accept-Language", "en-US,en")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: Zhipu Team API key or organization/project ID is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let plan = json.get("data")
        .and_then(|d| d.get("level"))
        .and_then(|v| v.as_str())
        .unwrap_or("Zhipu Team Plan")
        .to_string();

    let tiers = parse_zhipu_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::ZhipuTeam,
        plan,
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_minimax(
    client: &reqwest::Client,
    api_key: &str,
    region: &str, // "cn" | "global"
) -> Result<ProviderQuota, String> {
    let api_domain = if region == "cn" {
        "api.minimaxi.com"
    } else {
        "api.minimax.io"
    };
    let url = format!("https://{api_domain}/v1/api/openplatform/coding_plan/remains");

    let (status, raw, text) = fetch_response_bytes(
        client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: MiniMax API key is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let tiers = parse_minimax_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Minimax,
        plan: "MiniMax Coding Plan".to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_zenmux(
    client: &reqwest::Client,
    quota_url: &str,
    api_key: &str,
) -> Result<ProviderQuota, String> {
    // Validate quota_url
    let url = url::Url::parse(quota_url).map_err(|e| format!("Invalid ZenMux URL: {e}"))?;
    if url.scheme() != "https" {
        return Err("ZenMux quota URL must use HTTPS.".to_string());
    }
    let host = url.host_str().unwrap_or("");
    if host != "zenmux.com" && !host.ends_with(".zenmux.com") {
        return Err("ZenMux quota URL must belong to zenmux. domain.".to_string()).into();
    }
    if url.username() != "" || url.password().is_some() {
        return Err("Credentials in the URL are not allowed.".to_string());
    }
    if url.fragment().is_some() {
        return Err("URL fragments are not allowed.".to_string());
    }

    let (status, raw, text) = fetch_response_bytes(
        client
            .get(quota_url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15)),
    )
    .await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("Authentication failed: ZenMux API key is invalid.".to_string());
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status}): {text}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let plan_tier = json.get("data")
        .and_then(|d| d.get("plan"))
        .and_then(|p| p.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("ZenMux");

    let tiers = parse_zenmux_quota_body(&json)?;

    Ok(ProviderQuota {
        provider: ProviderKind::Zenmux,
        plan: plan_tier.to_string(),
        tiers,
        queried_at: now_millis(),
    })
}

pub async fn query_volcengine(
    client: &reqwest::Client,
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
) -> Result<ProviderQuota, String> {
    let reg = if region.is_empty() {
        VOLCENGINE_DEFAULT_REGION
    } else {
        region
    };

    // 1) Agent Plan：GetAFPUsage
    match volcengine_openapi_call(client, reg, access_key_id, secret_access_key, "GetAFPUsage").await {
        VolcCall::Auth(detail) => return Err(detail),
        VolcCall::Transient(detail) => return Err(format!("GetAFPUsage network failed: {detail}")),
        VolcCall::Soft(_) => {
            // Fallback to Coding Plan
        }
        VolcCall::Body(body) => {
            let result = body.get("Result").unwrap_or(&body);
            let tiers = parse_afp_tiers(result);
            if !tiers.is_empty() {
                let plan = result
                    .get("PlanType")
                    .and_then(|v| v.as_str())
                    .map(|s| format!("Agent Plan {}", s.trim()))
                    .unwrap_or_else(|| "Agent Plan".to_string());
                return Ok(ProviderQuota {
                    provider: ProviderKind::Volcengine,
                    plan,
                    tiers,
                    queried_at: now_millis(),
                });
            }
        }
    }

    // 2) Coding Plan: GetCodingPlanUsage
    match volcengine_openapi_call(client, reg, access_key_id, secret_access_key, "GetCodingPlanUsage").await {
        VolcCall::Auth(detail) => Err(detail),
        VolcCall::Transient(detail) => Err(format!("GetCodingPlanUsage network failed: {detail}")),
        VolcCall::Soft(detail) => Err(format!("API error: {detail}")),
        VolcCall::Body(body) => {
            let result = body.get("Result").unwrap_or(&body);
            let tiers = parse_coding_plan_tiers(result);
            if !tiers.is_empty() {
                return Ok(ProviderQuota {
                    provider: ProviderKind::Volcengine,
                    plan: "Coding Plan".to_string(),
                    tiers,
                    queried_at: now_millis(),
                });
            }
            Err("No active Agent Plan or Coding Plan subscription found for Volcengine.".to_string())
        }
    }
}

// ── Shared dispatcher for provider query ────────────────────────

pub async fn query_provider_quota(
    _provider: ProviderKind,
    config: &ProviderConfig,
    secret_store: &dyn SecretStore,
    account_id: &str,
    client: &reqwest::Client,
    gemini_token_cache: &Mutex<HashMap<String, (String, i64)>>,
) -> Result<ProviderQuota, String> {
    match config {
        ProviderConfig::Claude => {
            // For Claude, if source is CLI Auto, check CLI credentials
            if account_id.starts_with("cli:") {
                let creds = read_claude_cli_credentials();
                match creds.status {
                    CredentialStatus::Valid | CredentialStatus::Expired => {
                        let token = creds.token.ok_or_else(|| "Missing Claude token".to_string())?;
                        query_claude(client, &token).await
                    }
                    _ => Err(creds.message.unwrap_or_else(|| "Claude CLI credentials not found.".to_string())),
                }
            } else {
                let token = secret_store.get_secret(account_id, "api_key")?
                    .ok_or_else(|| "Claude API Key not found in secret store.".to_string())?;
                query_claude(client, &token).await
            }
        }
        ProviderConfig::Codex => {
            if account_id.starts_with("cli:") {
                let creds = read_codex_cli_credentials();
                match creds.status {
                    CredentialStatus::Valid | CredentialStatus::Expired => {
                        let token = creds.token.ok_or_else(|| "Missing Codex token".to_string())?;
                        query_codex(client, &token, creds.account_id.as_deref()).await
                    }
                    _ => Err(creds.message.unwrap_or_else(|| "Codex CLI credentials not found.".to_string())),
                }
            } else {
                let token = secret_store.get_secret(account_id, "api_key")?
                    .ok_or_else(|| "Codex token not found in secret store.".to_string())?;
                query_codex(client, &token, None).await
            }
        }
        ProviderConfig::Gemini => {
            if account_id.starts_with("cli:") {
                let creds = read_gemini_cli_credentials();
                match creds.status {
                    CredentialStatus::Valid => {
                        let token = creds.token.ok_or_else(|| "Missing Gemini token".to_string())?;
                        query_gemini(client, &token).await
                    }
                    CredentialStatus::Expired => {
                        if let Some(rt) = &creds.refresh_token {
                            // Check cache
                            let cached = {
                                let cache = gemini_token_cache.lock();
                                cache.get(rt).cloned()
                            };

                            let token = if let Some((tok, exp)) = cached {
                                if exp > now_millis() {
                                    tok
                                } else {
                                    let (new_tok, new_exp) = refresh_gemini_token(client, rt).await?;
                                    let mut cache = gemini_token_cache.lock();
                                    cache.insert(rt.clone(), (new_tok.clone(), new_exp));
                                    new_tok
                                }
                            } else {
                                let (new_tok, new_exp) = refresh_gemini_token(client, rt).await?;
                                let mut cache = gemini_token_cache.lock();
                                cache.insert(rt.clone(), (new_tok.clone(), new_exp));
                                new_tok
                            };
                            query_gemini(client, &token).await
                        } else {
                            let token = creds.token.ok_or_else(|| "Missing Gemini token".to_string())?;
                            query_gemini(client, &token).await
                        }
                    }
                    _ => Err(creds.message.unwrap_or_else(|| "Gemini CLI credentials not found.".to_string())),
                }
            } else {
                let token = secret_store.get_secret(account_id, "api_key")?
                    .ok_or_else(|| "Gemini API Key not found in secret store.".to_string())?;
                query_gemini(client, &token).await
            }
        }
        ProviderConfig::Copilot { github_domain } => {
            let token = secret_store.get_secret(account_id, "github_token")?
                .ok_or_else(|| "Copilot token not found in secret store.".to_string())?;
            query_copilot(client, &token, github_domain.as_deref()).await
        }
        ProviderConfig::Kimi => {
            let api_key = secret_store.get_secret(account_id, "api_key")?
                .ok_or_else(|| "Kimi API Key not found in secret store.".to_string())?;
            query_kimi(client, &api_key).await
        }
        ProviderConfig::Zhipu { region } => {
            let api_key = secret_store.get_secret(account_id, "api_key")?
                .ok_or_else(|| "Zhipu API Key not found in secret store.".to_string())?;
            query_zhipu(client, &api_key, region).await
        }
        ProviderConfig::ZhipuTeam { organization_id, project_id } => {
            let api_key = secret_store.get_secret(account_id, "api_key")?
                .ok_or_else(|| "Zhipu Team API Key not found in secret store.".to_string())?;
            query_zhipu_team(client, &api_key, organization_id, project_id).await
        }
        ProviderConfig::Minimax { region } => {
            let api_key = secret_store.get_secret(account_id, "api_key")?
                .ok_or_else(|| "MiniMax API Key not found in secret store.".to_string())?;
            query_minimax(client, &api_key, region).await
        }
        ProviderConfig::Zenmux { quota_url } => {
            let api_key = secret_store.get_secret(account_id, "api_key")?
                .ok_or_else(|| "ZenMux API Key not found in secret store.".to_string())?;
            query_zenmux(client, quota_url, &api_key).await
        }
        ProviderConfig::Volcengine { region } => {
            let access_key_id = secret_store.get_secret(account_id, "access_key_id")?
                .ok_or_else(|| "Volcengine AccessKey ID not found in secret store.".to_string())?;
            let secret_access_key = secret_store.get_secret(account_id, "secret_access_key")?
                .ok_or_else(|| "Volcengine SecretAccessKey not found in secret store.".to_string())?;
            query_volcengine(client, &access_key_id, &secret_access_key, region).await
        }
    }
}
