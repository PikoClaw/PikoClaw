// OAuth 2.0 constants — mirrors claude-code/constants/oauth.ts (production config)

/// Anthropic OAuth client ID (same client ID as Claude Code).
pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Authorization endpoint (Console path, redirects to claude.ai when needed).
pub const CONSOLE_AUTHORIZE_URL: &str = "https://platform.claude.com/oauth/authorize";

/// Authorization endpoint via claude.com attribution bounce (automatic browser flow).
pub const CLAUDE_AI_AUTHORIZE_URL: &str = "https://claude.com/cai/oauth/authorize";

/// Token endpoint for code exchange and refresh.
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

/// Used to create an API key for Console (pay-per-use) users.
pub const API_KEY_URL: &str = "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";

/// Redirect URI for the manual (paste-code) flow.
pub const MANUAL_REDIRECT_URL: &str = "https://platform.claude.com/oauth/code/callback";

/// Success page shown after automatic browser auth.
pub const CLAUDEAI_SUCCESS_URL: &str =
    "https://platform.claude.com/oauth/code/success?app=claude-code";

/// All OAuth scopes requested — union of Console and Claude.ai scopes.
pub const ALL_SCOPES: &str =
    "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

/// Scope that indicates the user is a Claude.ai subscriber (inference access).
pub const INFERENCE_SCOPE: &str = "user:inference";

/// Build the authorization URL for the browser.
///
/// * `is_manual = true`  → redirect_uri points to Anthropic's hosted callback page
/// * `is_manual = false` → redirect_uri points to the local callback server
pub fn build_auth_url(challenge: &str, state: &str, port: u16, is_manual: bool) -> String {
    let redirect_uri = if is_manual {
        MANUAL_REDIRECT_URL.to_string()
    } else {
        format!("http://localhost:{port}/callback")
    };

    let base = CONSOLE_AUTHORIZE_URL;
    format!(
        "{base}?code=true\
         &client_id={CLIENT_ID}\
         &response_type=code\
         &redirect_uri={redirect_uri}\
         &scope={ALL_SCOPES}\
         &code_challenge={challenge}\
         &code_challenge_method=S256\
         &state={state}",
        redirect_uri = urlencoded(&redirect_uri),
        ALL_SCOPES = urlencoded(ALL_SCOPES),
    )
}

/// Minimal percent-encoding for URL query parameter values.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
