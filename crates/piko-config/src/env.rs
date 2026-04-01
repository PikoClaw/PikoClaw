use std::env;

pub fn anthropic_api_key() -> Option<String> {
    env::var("ANTHROPIC_API_KEY").ok()
}

pub fn anthropic_base_url() -> Option<String> {
    env::var("ANTHROPIC_BASE_URL").ok()
}

pub fn anthropic_model() -> Option<String> {
    env::var("ANTHROPIC_MODEL").ok()
}

pub fn pikoclaw_config_path() -> Option<String> {
    env::var("PIKOCLAW_CONFIG").ok()
}
