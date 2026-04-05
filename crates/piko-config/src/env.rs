use std::env;

pub fn anthropic_api_key() -> Option<String> {
    env::var("ANTHROPIC_API_KEY").ok()
}

/// Bearer token alternative to ANTHROPIC_API_KEY (e.g. OpenRouter via ANTHROPIC_AUTH_TOKEN).
/// When set, requests use `Authorization: Bearer <token>` instead of `x-api-key`.
pub fn anthropic_auth_token() -> Option<String> {
    env::var("ANTHROPIC_AUTH_TOKEN").ok()
}

pub fn anthropic_base_url() -> Option<String> {
    env::var("ANTHROPIC_BASE_URL").ok()
}

pub fn anthropic_model() -> Option<String> {
    env::var("ANTHROPIC_MODEL").ok()
}

/// Model override for the default Sonnet slot (mirrors claude-code's ANTHROPIC_DEFAULT_SONNET_MODEL).
pub fn anthropic_default_sonnet_model() -> Option<String> {
    env::var("ANTHROPIC_DEFAULT_SONNET_MODEL").ok()
}

pub fn pikoclaw_config_path() -> Option<String> {
    env::var("PIKOCLAW_CONFIG").ok()
}
