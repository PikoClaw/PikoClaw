mod cli;

use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Commands};
use piko_agent::agent::{Agent, AgentConfig};
use piko_api::ModelRegistry;
use piko_config::loader::{load_config, save_config};
use piko_session::fs_store::FilesystemSessionStore;
use piko_session::store::SessionStore;
use piko_skills::dispatcher::SkillDispatcher;
use piko_skills::loader::load_user_skills;
use piko_skills::registry::SkillRegistry;
use piko_tui::app::App;
use piko_types::ProviderId;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_target(false)
        .without_time()
        .init();

    let mut config = load_config()?;

    // ── First-run onboarding ───────────────────────────────────────────────
    // Show the theme picker whenever onboarding hasn't been completed yet.
    // (Same trigger as claude-code: missing theme or flag not set.)
    if !config.tui.has_completed_onboarding {
        let chosen = piko_tui::onboarding::run_theme_picker()?;
        config.tui.theme = chosen.to_string();
        config.tui.has_completed_onboarding = true;
        // Best-effort save; don't abort if the write fails (e.g. read-only FS).
        let _ = save_config(&config);
    }

    if let Some(ref model) = cli.model {
        config.api.model = piko_types::model::ModelId::from_alias(model);
    }

    let mut model_registry = ModelRegistry::new();
    // Load cached models.dev data (populated by background refresh on previous runs).
    if let Some(cache) = models_cache_path() {
        model_registry.load_cache(&cache);
    }
    // Spawn background refresh so the next run has fresh data.
    spawn_models_cache_refresh();
    let inferred_provider = config.api.provider.clone().or_else(|| {
        model_registry
            .find_provider_for_model(config.api.model.as_str())
            .map(|p| p.to_string())
    });
    config.api.provider = inferred_provider.clone();

    if inferred_provider.as_deref() == Some(ProviderId::OPENROUTER) {
        if config.api.base_url == "https://api.anthropic.com" {
            config.api.base_url = "https://openrouter.ai/api".to_string();
        }
        if config.api.auth_token.is_none() {
            if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
                if !key.is_empty() {
                    config.api.auth_token = Some(key);
                }
            }
        }
        if let Some((provider, model)) = config.api.model.as_str().split_once('/') {
            if provider == ProviderId::OPENROUTER {
                config.api.model = model.into();
            }
        }
    } else if inferred_provider.as_deref() == Some(ProviderId::OPENAI) {
        if let Some(base_url) = piko_config::env::openai_base_url() {
            config.api.base_url = base_url;
        } else if config.api.base_url == "https://api.anthropic.com" {
            config.api.base_url = "https://api.openai.com".to_string();
        }
        if let Some(key) = piko_config::env::openai_api_key() {
            config.api.api_key = Some(key);
        }
        config.api.auth_token = None;
        if let Some((provider, model)) = config.api.model.as_str().split_once('/') {
            if provider == ProviderId::OPENAI {
                config.api.model = model.into();
            }
        }
    }

    // Resolve credential following the same priority as claude-code:
    //   1. ANTHROPIC_AUTH_TOKEN  → Bearer-token auth (e.g. OpenRouter)
    //   2. ANTHROPIC_API_KEY     → standard x-api-key auth
    //   3. Stored OAuth tokens   → refresh silently or run browser login flow
    // ANTHROPIC_BASE_URL is already applied to config.api.base_url by load_config().
    let (credential, use_bearer_auth) = if inferred_provider.as_deref() == Some(ProviderId::OPENAI)
    {
        let key = config
            .api
            .api_key
            .clone()
            .ok_or_else(|| anyhow!("OPENAI_API_KEY is required for provider 'openai'"))?;
        (key, true)
    } else if let Some(token) = config.api.auth_token.clone() {
        (token, true)
    } else if let Some(key) = config.api.api_key.clone() {
        (key, false)
    } else {
        // No explicit credentials — try stored OAuth tokens or run browser login.
        let tokens = piko_oauth::run_login_flow().await?;
        // Console users get an API key written into config by run_login_flow;
        // claude.ai subscribers get a Bearer access token.
        let use_bearer = tokens.refresh_token.is_some() || tokens.expires_at_ms != u64::MAX;
        (tokens.access_token, use_bearer)
    };

    let base_url = config.api.base_url.clone();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match cli.command {
        Some(Commands::Resume { ref session_id }) => {
            let agent_config = build_agent_config(&config, &cli, cwd.clone());
            let mut agent = Agent::with_options(
                agent_config,
                &credential,
                &base_url,
                use_bearer_auth,
                inferred_provider.as_deref(),
            )?;
            let store = Arc::new(FilesystemSessionStore::with_default_path());

            let session = SessionStore::load(store.as_ref(), session_id)
                .await?
                .ok_or_else(|| anyhow!("session '{}' not found", session_id))?;

            agent = agent.with_session_store(store).with_session(session);

            run_interactive(agent, &cli, &config, model_registry).await
        }
        Some(Commands::Continue) => {
            let agent_config = build_agent_config(&config, &cli, cwd.clone());
            let mut agent = Agent::with_options(
                agent_config,
                &credential,
                &base_url,
                use_bearer_auth,
                inferred_provider.as_deref(),
            )?;
            let store = Arc::new(FilesystemSessionStore::with_default_path());

            if let Some(session) =
                SessionStore::latest_for_cwd(store.as_ref(), &cwd.to_string_lossy()).await?
            {
                agent = agent.with_session_store(store).with_session(session);
            } else {
                let session = piko_session::session::Session::new(
                    cwd.to_string_lossy().to_string(),
                    config.api.model.as_str(),
                );
                agent = agent.with_session_store(store).with_session(session);
            }

            run_interactive(agent, &cli, &config, model_registry).await
        }
        None => {
            if let Some(ref prompt) = cli.print {
                let agent_config = build_agent_config(&config, &cli, cwd);
                let mut agent = Agent::with_options(
                    agent_config,
                    &credential,
                    &base_url,
                    use_bearer_auth,
                    inferred_provider.as_deref(),
                )?;
                agent.run_print(prompt).await?;
                println!();
                Ok(())
            } else {
                let agent_config = build_agent_config(&config, &cli, cwd.clone());
                let mut agent = Agent::with_options(
                    agent_config,
                    &credential,
                    &base_url,
                    use_bearer_auth,
                    inferred_provider.as_deref(),
                )?;
                let store = Arc::new(FilesystemSessionStore::with_default_path());
                let session = piko_session::session::Session::new(
                    cwd.to_string_lossy().to_string(),
                    config.api.model.as_str(),
                );
                agent = agent.with_session_store(store).with_session(session);
                run_interactive(agent, &cli, &config, model_registry).await
            }
        }
    }
}

async fn run_interactive(
    agent: Agent,
    cli: &Cli,
    config: &piko_config::config::PikoConfig,
    model_registry: ModelRegistry,
) -> Result<()> {
    let mut skill_registry = SkillRegistry::with_built_ins();
    let _ = load_user_skills(&mut skill_registry);
    let dispatcher = SkillDispatcher::new(skill_registry);

    let budget = cli.max_budget_usd.or(config.api.max_budget_usd);
    let provider_id = config.api.provider.as_deref().unwrap_or("anthropic");
    let provider_name = match provider_id {
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        "google" => "Google",
        _ => "Anthropic",
    };
    let mut app = App::new(
        agent,
        dispatcher,
        &config.tui.theme,
        provider_name,
        provider_id,
        model_registry,
        budget,
    );
    app.run().await
}

fn build_agent_config(
    config: &piko_config::config::PikoConfig,
    cli: &Cli,
    cwd: PathBuf,
) -> AgentConfig {
    let extended_thinking = cli.thinking || config.api.extended_thinking;
    let thinking_budget_tokens = cli
        .thinking_budget
        .unwrap_or(config.api.thinking_budget_tokens);
    AgentConfig {
        model: config.api.model.clone(),
        max_tokens: config.api.max_tokens,
        max_turns: cli.max_turns,
        cwd,
        system_prompt: cli.system_prompt.clone(),
        bypass_permissions: cli.dangerously_skip_permissions,
        extended_thinking,
        thinking_budget_tokens,
    }
}

fn models_cache_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "pikoclaw", "pikoclaw")
        .map(|d| d.cache_dir().join("models.json"))
}

fn spawn_models_cache_refresh() {
    let cache_path = match models_cache_path() {
        Some(p) => p,
        None => return,
    };
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let url = std::env::var("MODELS_DEV_URL")
            .unwrap_or_else(|_| "https://models.dev/api.json".to_string());
        if let Ok(resp) = client
            .get(&url)
            .header("User-Agent", "PikoClaw")
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Some(parent) = cache_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&cache_path, &text);
                }
            }
        }
    });
}
