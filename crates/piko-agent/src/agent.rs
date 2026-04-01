use crate::agent_loop::run_turn;
use crate::context::ConversationContext;
use crate::output::{OutputSink, StdoutSink};
use anyhow::Result;
use piko_api::AnthropicClient;
use piko_config::config::PikoConfig;
use piko_permissions::checker::PermissionChecker;
use piko_permissions::default::DefaultPermissionChecker;
use piko_permissions::policy::PermissionPolicy;
use piko_session::session::Session;
use piko_session::store::SessionStore;
use piko_tools::registry::ToolRegistry;
use piko_types::model::ModelId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct AgentConfig {
    pub model: ModelId,
    pub max_tokens: u32,
    pub max_turns: Option<usize>,
    pub cwd: PathBuf,
    pub system_prompt: Option<String>,
    pub bypass_permissions: bool,
}

impl AgentConfig {
    pub fn from_pikoclaw_config(config: &PikoConfig, cwd: PathBuf) -> Self {
        Self {
            model: config.api.model.clone(),
            max_tokens: config.api.max_tokens,
            max_turns: None,
            cwd,
            system_prompt: None,
            bypass_permissions: false,
        }
    }
}

pub struct Agent {
    pub config: AgentConfig,
    pub client: Arc<AnthropicClient>,
    pub tools: Arc<ToolRegistry>,
    pub permissions: Arc<dyn PermissionChecker>,
    pub session_store: Option<Arc<dyn SessionStore>>,
    pub context: ConversationContext,
    pub session: Option<Session>,
    pub cancellation: CancellationToken,
}

impl Agent {
    pub fn new(config: AgentConfig, api_key: impl Into<String>) -> Result<Self> {
        let client = Arc::new(AnthropicClient::new(api_key)?);
        let mut tools = ToolRegistry::with_defaults();

        let agent_config = Arc::new(AgentConfig {
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            max_turns: config.max_turns,
            cwd: config.cwd.clone(),
            system_prompt: config.system_prompt.clone(),
            bypass_permissions: config.bypass_permissions,
        });
        tools.register(Arc::new(crate::agent_tool::AgentTool::new(
            Arc::clone(&client),
            agent_config,
        )));

        let policy =
            PermissionPolicy::from_config(&piko_config::config::PermissionsConfig::default());
        let permissions: Arc<dyn PermissionChecker> = if config.bypass_permissions {
            Arc::new(DefaultPermissionChecker::bypass())
        } else {
            Arc::new(DefaultPermissionChecker::new(policy))
        };

        let mut context = ConversationContext::new();
        let claude_md = piko_config::load_claude_md(&config.cwd);
        let system = match (&config.system_prompt, claude_md) {
            (Some(custom), Some(md)) => Some(format!("{}\n\n{}", custom, md)),
            (Some(custom), None) => Some(custom.clone()),
            (None, Some(md)) => Some(md),
            (None, None) => None,
        };
        context.system_prompt = system;

        Ok(Self {
            config,
            client,
            tools: Arc::new(tools),
            permissions,
            session_store: None,
            context,
            session: None,
            cancellation: CancellationToken::new(),
        })
    }

    pub fn with_session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    pub fn with_permission_checker(mut self, checker: Arc<dyn PermissionChecker>) -> Self {
        self.permissions = checker;
        self
    }

    pub fn with_session(mut self, session: Session) -> Self {
        self.context.set_messages(session.messages.clone());
        self.session = Some(session);
        self
    }

    pub async fn run_print(&mut self, prompt: &str) -> Result<String> {
        let sink = Arc::new(StdoutSink);
        self.run_turn(prompt, sink.clone()).await
    }

    pub async fn run_turn(&mut self, prompt: &str, sink: Arc<dyn OutputSink>) -> Result<String> {
        self.context.push_user(prompt);

        let result = run_turn(
            &self.client,
            &self.tools,
            &*self.permissions,
            &mut self.context,
            &self.config,
            sink,
            self.cancellation.clone(),
        )
        .await?;

        if let Some(ref mut session) = self.session {
            session.messages = self.context.messages.clone();
            session.touch();
            if let Some(ref store) = self.session_store {
                if let Err(e) = store.save(session).await {
                    tracing::warn!("failed to save session: {}", e);
                }
            }
        }

        Ok(result)
    }
}
