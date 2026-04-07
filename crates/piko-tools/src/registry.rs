use crate::tool_trait::Tool;
use piko_types::tool::ToolDefinition;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|t| !t.is_web_search())
            .map(|t| t.definition())
            .collect()
    }

    pub fn has_web_search(&self) -> bool {
        self.tools.values().any(|t| t.is_web_search())
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    pub fn with_defaults() -> Self {
        use crate::todo_write::{TodoStore, TodoWriteTool};
        use crate::{
            apply_patch::ApplyPatchTool,
            bash::BashTool,
            batch_edit::BatchEditTool,
            brief::BriefTool,
            cron::{CronCreateTool, CronDeleteTool, CronListTool},
            file_edit::FileEditTool,
            file_read::FileReadTool,
            file_write::FileWriteTool,
            glob::GlobTool,
            grep::GrepTool,
            notebook_edit::NotebookEditTool,
            powershell::PowerShellTool,
            remote_trigger::RemoteTriggerTool,
            send_message::SendMessageTool,
            sleep::SleepTool,
            synthetic_output::SyntheticOutputTool,
            tasks::{
                TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskStopTool,
                TaskUpdateTool,
            },
            tool_search::ToolSearchTool,
            worktree::{EnterWorktreeTool, ExitWorktreeTool},
        };
        use std::collections::HashMap;

        let mut registry = Self::new();
        registry.register(Arc::new(BashTool));
        registry.register(Arc::new(FileReadTool));
        registry.register(Arc::new(FileWriteTool));
        registry.register(Arc::new(FileEditTool));
        registry.register(Arc::new(GlobTool));
        registry.register(Arc::new(GrepTool));
        registry.register(Arc::new(crate::web_fetch::WebFetchTool::new()));
        registry.register(Arc::new(crate::web_search::WebSearchTool));
        registry.register(Arc::new(NotebookEditTool));
        let todo_store: TodoStore = Arc::new(std::sync::Mutex::new(HashMap::new()));
        registry.register(Arc::new(TodoWriteTool::new(todo_store)));
        registry.register(Arc::new(ApplyPatchTool));
        registry.register(Arc::new(BatchEditTool));
        registry.register(Arc::new(BriefTool));
        registry.register(Arc::new(SleepTool));
        registry.register(Arc::new(SyntheticOutputTool));
        registry.register(Arc::new(TaskCreateTool));
        registry.register(Arc::new(TaskGetTool));
        registry.register(Arc::new(TaskUpdateTool));
        registry.register(Arc::new(TaskListTool));
        registry.register(Arc::new(TaskStopTool));
        registry.register(Arc::new(TaskOutputTool));
        registry.register(Arc::new(SendMessageTool));
        registry.register(Arc::new(PowerShellTool));
        registry.register(Arc::new(RemoteTriggerTool));
        registry.register(Arc::new(ToolSearchTool));
        registry.register(Arc::new(EnterWorktreeTool));
        registry.register(Arc::new(ExitWorktreeTool));
        registry.register(Arc::new(CronCreateTool));
        registry.register(Arc::new(CronDeleteTool));
        registry.register(Arc::new(CronListTool));
        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
