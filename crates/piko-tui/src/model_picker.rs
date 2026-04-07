// ---------------------------------------------------------------------------
// ModelEntry
// ---------------------------------------------------------------------------

/// A single model entry shown in the picker.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// Whether this is the currently active model.
    pub is_current: bool,
}

// ---------------------------------------------------------------------------
// Provider-aware model lists
// ---------------------------------------------------------------------------

/// Helper to build a `ModelEntry` with `is_current = false`.
fn model_entry(id: &str, name: &str, desc: &str) -> ModelEntry {
    ModelEntry {
        id: id.to_string(),
        display_name: name.to_string(),
        description: desc.to_string(),
        is_current: false,
    }
}

/// Get models for a provider from the model registry (models.dev data).
///
/// Falls back to the hardcoded `models_for_provider()` list when the registry
/// has no entries for this provider.  This makes models.dev the single source
/// of truth once the background fetch completes, while still providing a good
/// experience before the fetch finishes.
pub fn models_for_provider_from_registry(
    provider_id: &str,
    registry: &piko_api::ModelRegistry,
) -> Vec<ModelEntry> {
    let entries = registry.list_by_provider(provider_id);
    if !entries.is_empty() {
        entries
            .iter()
            .map(|e| {
                let ctx_k = e.info.context_window / 1000;
                let cost_str = match (e.cost_input, e.cost_output) {
                    (Some(ci), Some(co)) => format!("{}K ctx | ${:.2}/${:.2} per M", ctx_k, ci, co),
                    _ => format!("{}K ctx", ctx_k),
                };
                ModelEntry {
                    id: e.info.id.to_string(),
                    display_name: e.info.name.clone(),
                    description: cost_str,
                    is_current: false,
                }
            })
            .collect()
    } else {
        // Fall back to hardcoded
        models_for_provider(provider_id)
    }
}

/// Build the model list for a given provider.
///
/// Returns a curated set of well-known models for major providers so the
/// `/model` picker shows relevant choices regardless of whether the API
/// returned a live model list.
pub fn models_for_provider(provider_id: &str) -> Vec<ModelEntry> {
    match provider_id {
        "anthropic" => vec![
            model_entry(
                "claude-opus-4-6",
                "Claude Opus 4.6",
                "Most capable — best for complex reasoning and analysis",
            ),
            model_entry(
                "claude-sonnet-4-6",
                "Claude Sonnet 4.6",
                "Balanced performance and speed — great for coding tasks",
            ),
            model_entry(
                "claude-haiku-4-5-20251001",
                "Claude Haiku 4.5",
                "Fast and efficient — ideal for quick completions",
            ),
        ],
        "openai" => vec![
            model_entry("gpt-4o", "GPT-4o", "128K context"),
            model_entry("gpt-4o-mini", "GPT-4o mini", "128K context"),
            model_entry("gpt-4.1", "GPT-4.1", "1M context"),
            model_entry("gpt-4.1-mini", "GPT-4.1 mini", "1M context"),
            model_entry("gpt-4.1-nano", "GPT-4.1 nano", "1M context"),
            model_entry("o3", "o3", "200K context"),
            model_entry("o3-mini", "o3 mini", "200K context"),
            model_entry("o4-mini", "o4 mini", "200K context"),
            model_entry("gpt-4-turbo", "GPT-4 Turbo", "128K context"),
        ],
        "google" => vec![
            model_entry("gemini-2.5-pro", "Gemini 2.5 Pro", "1M context"),
            model_entry("gemini-2.5-flash", "Gemini 2.5 Flash", "1M context"),
            model_entry("gemini-2.0-flash", "Gemini 2.0 Flash", "1M context"),
        ],
        "groq" => vec![
            model_entry("llama-3.3-70b-versatile", "Llama 3.3 70B", "128K context"),
            model_entry("llama-3.1-8b-instant", "Llama 3.1 8B", "128K context"),
            model_entry("mixtral-8x7b-32768", "Mixtral 8x7B", "32K context"),
            model_entry("gemma2-9b-it", "Gemma 2 9B", "8K context"),
        ],
        "cerebras" => vec![
            model_entry("llama-3.3-70b", "Llama 3.3 70B", "128K context"),
            model_entry("llama-3.1-8b", "Llama 3.1 8B", "128K context"),
        ],
        "deepseek" => vec![
            model_entry("deepseek-chat", "DeepSeek V3", "64K context"),
            model_entry("deepseek-reasoner", "DeepSeek R1", "64K context"),
        ],
        "mistral" => vec![
            model_entry("mistral-large-latest", "Mistral Large", "128K context"),
            model_entry("mistral-small-latest", "Mistral Small", "128K context"),
            model_entry("codestral-latest", "Codestral", "32K context"),
        ],
        "xai" => vec![
            model_entry("grok-2", "Grok 2", "128K context"),
            model_entry("grok-3", "Grok 3", "128K context"),
            model_entry("grok-3-mini", "Grok 3 mini", "128K context"),
        ],
        "openrouter" => vec![
            model_entry(
                "anthropic/claude-sonnet-4",
                "Claude Sonnet 4",
                "via OpenRouter",
            ),
            model_entry("openai/gpt-4o", "GPT-4o", "via OpenRouter"),
            model_entry("google/gemini-2.5-pro", "Gemini 2.5 Pro", "via OpenRouter"),
            model_entry(
                "meta-llama/llama-3.3-70b-instruct",
                "Llama 3.3 70B",
                "via OpenRouter",
            ),
        ],
        "cohere" => vec![
            model_entry("command-r-plus", "Command R+", "128K context"),
            model_entry("command-r", "Command R", "128K context"),
        ],
        "perplexity" => vec![
            model_entry("sonar-pro", "Sonar Pro", "search-augmented"),
            model_entry("sonar", "Sonar", "search-augmented"),
        ],
        "togetherai" | "together-ai" => vec![
            model_entry(
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "Llama 3.3 70B Turbo",
                "128K context",
            ),
            model_entry(
                "meta-llama/Llama-3.1-8B-Instruct-Turbo",
                "Llama 3.1 8B Turbo",
                "128K context",
            ),
            model_entry(
                "Qwen/Qwen2.5-72B-Instruct-Turbo",
                "Qwen 2.5 72B Turbo",
                "128K context",
            ),
        ],
        "deepinfra" => vec![
            model_entry(
                "meta-llama/Llama-3.3-70B-Instruct",
                "Llama 3.3 70B",
                "128K context",
            ),
            model_entry(
                "meta-llama/Llama-3.1-8B-Instruct",
                "Llama 3.1 8B",
                "128K context",
            ),
        ],
        "ollama" => vec![
            model_entry("llama3.2", "Llama 3.2", "local"),
            model_entry("mistral", "Mistral", "local"),
            model_entry("codellama", "Code Llama", "local"),
            model_entry("gemma2", "Gemma 2", "local"),
            model_entry("phi3", "Phi-3", "local"),
            model_entry("qwen2.5", "Qwen 2.5", "local"),
        ],
        "azure" => vec![
            model_entry("gpt-4o", "GPT-4o (Azure)", "128K context"),
            model_entry("gpt-4o-mini", "GPT-4o mini (Azure)", "128K context"),
        ],
        "amazon-bedrock" => vec![
            model_entry(
                "anthropic.claude-sonnet-4-6-v1",
                "Claude Sonnet 4.6 (Bedrock)",
                "200K context",
            ),
            model_entry(
                "anthropic.claude-haiku-4-5-20251001-v1",
                "Claude Haiku 4.5 (Bedrock)",
                "200K context",
            ),
        ],
        _ => vec![model_entry("default", "Default model", "")],
    }
}
