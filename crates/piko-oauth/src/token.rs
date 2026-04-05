// OAuth token exchange and refresh — mirrors claude-code/services/oauth/client.ts

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::{CLIENT_ID, TOKEN_URL};
use crate::storage::StoredTokens;

const REFRESH_SCOPES: &str =
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

#[derive(Deserialize)]
struct RawTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64, // seconds
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Returns `true` if the token expires within the next 5 minutes.
pub fn is_expired(expires_at_ms: u64) -> bool {
    const BUFFER_MS: u64 = 5 * 60 * 1_000;
    now_ms() + BUFFER_MS >= expires_at_ms
}

/// Exchange an authorization code for tokens (RFC 6749 §4.1.3 + PKCE).
pub async fn exchange_code(
    code: &str,
    state: &str,
    code_verifier: &str,
    port: u16,
) -> Result<StoredTokens> {
    let redirect_uri = format!("http://localhost:{port}/callback");

    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": redirect_uri,
        "client_id": CLIENT_ID,
        "code_verifier": code_verifier,
        "state": state,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Token exchange request failed")?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => anyhow!("Authentication failed: invalid authorization code"),
            _ => anyhow!("Token exchange failed ({status}): {text}"),
        });
    }

    let raw: RawTokenResponse =
        serde_json::from_str(&text).context("Failed to parse token response")?;

    Ok(StoredTokens {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_at_ms: now_ms() + raw.expires_in * 1_000,
    })
}

/// Refresh an expired access token using the refresh token grant.
pub async fn refresh(refresh_token: &str) -> Result<StoredTokens> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": CLIENT_ID,
        "scope": REFRESH_SCOPES,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Token refresh request failed")?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(anyhow!("Token refresh failed ({status}): {text}"));
    }

    let raw: RawTokenResponse =
        serde_json::from_str(&text).context("Failed to parse refresh response")?;

    Ok(StoredTokens {
        access_token: raw.access_token,
        // RFC 6749: server may issue a new refresh token or re-use the old one
        refresh_token: raw.refresh_token.or(Some(refresh_token.to_string())),
        expires_at_ms: now_ms() + raw.expires_in * 1_000,
    })
}

/// Attempt to create an Anthropic API key from a Console OAuth access token.
/// Console users get pay-per-token billing via an API key; claude.ai subscribers
/// use the Bearer token directly.  Returns the API key if created successfully.
pub async fn create_api_key(access_token: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(crate::constants::API_KEY_URL)
        .header("Authorization", format!("Bearer {access_token}"))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("raw_key")?.as_str().map(str::to_string)
}
