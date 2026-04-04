mod cli;

use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Commands};
use piko_agent::agent::{Agent, AgentConfig};
use piko_config::loader::{load_config, save_config};
use piko_session::fs_store::FilesystemSessionStore;
use piko_session::store::SessionStore;
use piko_skills::dispatcher::SkillDispatcher;
use piko_skills::loader::load_user_skills;
use piko_skills::registry::SkillRegistry;
use piko_tui::app::App;
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

    let api_key = config
        .api
        .api_key
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            anyhow!("ANTHROPIC_API_KEY not set. Set it via environment variable or config file.")
        })?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match cli.command {
        Some(Commands::Resume { ref session_id }) => {
            let agent_config = build_agent_config(&config, &cli, cwd.clone());
            let mut agent = Agent::new(agent_config, &api_key)?;
            let store = Arc::new(FilesystemSessionStore::with_default_path());

            let session = SessionStore::load(store.as_ref(), session_id)
                .await?
                .ok_or_else(|| anyhow!("session '{}' not found", session_id))?;

            agent = agent.with_session_store(store).with_session(session);

            run_interactive(agent, &cli, &config).await
        }
        Some(Commands::Continue) => {
            let agent_config = build_agent_config(&config, &cli, cwd.clone());
            let mut agent = Agent::new(agent_config, &api_key)?;
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

            run_interactive(agent, &cli, &config).await
        }
        None => {
            if let Some(ref prompt) = cli.print {
                let agent_config = build_agent_config(&config, &cli, cwd);
                let mut agent = Agent::new(agent_config, &api_key)?;
                agent.run_print(prompt).await?;
                println!();
                Ok(())
            } else {
                let agent_config = build_agent_config(&config, &cli, cwd.clone());
                let mut agent = Agent::new(agent_config, &api_key)?;
                let store = Arc::new(FilesystemSessionStore::with_default_path());
                let session = piko_session::session::Session::new(
                    cwd.to_string_lossy().to_string(),
                    config.api.model.as_str(),
                );
                agent = agent.with_session_store(store).with_session(session);
                run_interactive(agent, &cli, &config).await
            }
        }
    }
}

async fn run_interactive(
    agent: Agent,
    cli: &Cli,
    config: &piko_config::config::PikoConfig,
) -> Result<()> {
    let mut skill_registry = SkillRegistry::with_built_ins();
    let _ = load_user_skills(&mut skill_registry);
    let dispatcher = SkillDispatcher::new(skill_registry);

    let budget = cli.max_budget_usd.or(config.api.max_budget_usd);
    let mut app = App::new(agent, dispatcher, &config.tui.theme, budget);
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
