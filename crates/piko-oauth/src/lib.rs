// PikoClaw OAuth 2.0 PKCE authentication
//
// Implements the same browser-based login flow as Claude Code:
//   1. Start a temporary localhost HTTP server
//   2. Build the authorization URL with PKCE challenge
//   3. Print the manual URL (fallback for headless/SSH environments)
//   4. Try to open the system browser automatically
//   5. Wait for the browser to redirect back to localhost
//   6. Exchange the authorization code for tokens
//   7. Persist tokens (macOS Keychain / plaintext file)
//
// On the next run, stored tokens are loaded and refreshed automatically
// without prompting the user again.

pub mod browser;
pub mod callback_server;
pub mod constants;
pub mod crypto;
pub mod storage;
pub mod token;

pub use storage::StoredTokens;

use anyhow::{anyhow, Result};
use callback_server::{extract_code_from_pasted, CallbackServer};
use std::time::Duration;

/// Entry point called from `main.rs` when no API key or auth token is configured.
///
/// Returns `StoredTokens` whose `access_token` should be used as a Bearer token
/// against `https://api.anthropic.com`.
pub async fn run_login_flow() -> Result<StoredTokens> {
    // ── 1. Check stored tokens ────────────────────────────────────────────────
    if let Some(tokens) = storage::load_tokens()? {
        if !tokens.is_expired() {
            return Ok(tokens);
        }
        // Attempt silent refresh before prompting the user
        if let Some(ref rt) = tokens.refresh_token {
            match token::refresh(rt).await {
                Ok(fresh) => {
                    storage::save_tokens(&fresh)?;
                    return Ok(fresh);
                }
                Err(e) => {
                    tracing::warn!("Silent token refresh failed ({e}), starting login flow");
                }
            }
        }
    }

    // ── 2. Start the localhost callback server ────────────────────────────────
    let server = CallbackServer::bind().await?;
    let port = server.port();

    // ── 3. Generate PKCE values ───────────────────────────────────────────────
    let verifier = crypto::generate_code_verifier();
    let challenge = crypto::generate_code_challenge(&verifier);
    let state = crypto::generate_state();

    let auto_url = constants::build_auth_url(&challenge, &state, port, false);
    let manual_url = constants::build_auth_url(&challenge, &state, port, true);

    // ── 4. Print instructions ─────────────────────────────────────────────────
    eprintln!();
    eprintln!("PikoClaw needs to authenticate with your Anthropic account.");
    eprintln!();
    eprintln!("Opening browser… If it doesn't open, visit this URL:");
    eprintln!("  {manual_url}");
    eprintln!();
    eprintln!("Waiting for authentication (timeout: 5 minutes)…");
    eprintln!("Press Ctrl+C to cancel.");
    eprintln!();

    // ── 5. Try to open the system browser ────────────────────────────────────
    let _ = browser::open(&auto_url).await;

    // ── 6. Wait for the callback or manual paste ──────────────────────────────
    let code = wait_for_code(server, &state).await?;

    eprintln!("Authentication received. Finalizing…");

    // ── 7. Exchange code for tokens ───────────────────────────────────────────
    let tokens = token::exchange_code(&code, &state, &verifier, port).await?;

    // ── 8. Console users: try to get an API key (optional, best-effort) ───────
    // claude.ai subscribers use the Bearer token directly; Console users get
    // a raw API key they can use without Bearer auth.  We try the API key path
    // first; if it fails (e.g. user is a claude.ai subscriber) we fall back to
    // the Bearer token — both work fine with piko-api's `use_bearer_auth` flag.
    if let Some(api_key) = token::create_api_key(&tokens.access_token).await {
        // Store the API key in the pikoclaw config file so it survives restarts
        // without needing OAuth tokens.
        if let Err(e) = persist_api_key(&api_key) {
            tracing::warn!("Could not save API key to config: {e}");
        }
        // Return a synthetic StoredTokens with the raw API key so main.rs
        // can treat it uniformly.  expires_at_ms=u64::MAX means "never expires".
        return Ok(StoredTokens {
            access_token: api_key,
            refresh_token: None,
            expires_at_ms: u64::MAX,
        });
    }

    // ── 9. Persist OAuth tokens ───────────────────────────────────────────────
    storage::save_tokens(&tokens)?;
    eprintln!("Logged in successfully!\n");

    Ok(tokens)
}

/// Delete stored credentials (used by a future `pikoclaw auth logout` command).
pub fn logout() -> Result<()> {
    storage::delete_tokens()
}

// ─── Internals ────────────────────────────────────────────────────────────────

/// Race: browser callback (automatic) vs. manual paste (fallback).
///
/// We wait up to 5 minutes for the browser redirect.  If it times out we ask
/// the user to paste the redirect URL from their browser's address bar.
async fn wait_for_code(server: CallbackServer, state: &str) -> Result<String> {
    let state_owned = state.to_string();

    match tokio::time::timeout(Duration::from_secs(300), server.wait_for_code(&state_owned)).await {
        // Browser redirect succeeded
        Ok(Ok(code)) => return Ok(code),
        Ok(Err(e)) => return Err(e),
        // Timed out — fall through to manual paste
        Err(_) => {}
    }

    // Manual fallback
    eprintln!();
    eprintln!("Browser callback timed out.");
    eprintln!("After you complete authentication, your browser will show a page with a URL.");
    eprintln!("Paste the full redirect URL here (it starts with https://platform.claude.com/oauth/code/callback?...):");
    eprint!("> ");

    let pasted = tokio::task::spawn_blocking(|| {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        Ok::<String, std::io::Error>(line.trim().to_string())
    })
    .await
    .map_err(|e| anyhow!("Task panicked: {e}"))??;

    extract_code_from_pasted(&pasted, state)
}

/// Write the API key returned by the Console OAuth flow into the pikoclaw
/// config file so it is available on the next run without re-authenticating.
fn persist_api_key(api_key: &str) -> Result<()> {
    use piko_config::loader::{load_config, save_config};

    let mut config = load_config()?;
    config.api.api_key = Some(api_key.to_string());
    save_config(&config)
}
