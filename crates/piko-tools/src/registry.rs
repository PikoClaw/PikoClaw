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
            bash::BashTool, file_edit::FileEditTool, file_read::FileReadTool,
            file_write::FileWriteTool, glob::GlobTool, grep::GrepTool,
            notebook_edit::NotebookEditTool,
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
        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
