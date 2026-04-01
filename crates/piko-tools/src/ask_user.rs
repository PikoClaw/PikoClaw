use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

pub struct AskQuestion {
    pub question: String,
    pub options: Vec<String>,
    pub reply: oneshot::Sender<String>,
}

pub type AskUserTx = mpsc::UnboundedSender<AskQuestion>;

pub struct AskUserQuestionTool {
    tx: Arc<Mutex<Option<AskUserTx>>>,
}

impl AskUserQuestionTool {
    pub fn new(tx: AskUserTx) -> Self {
        Self {
            tx: Arc::new(Mutex::new(Some(tx))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct QuestionOption {
    label: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QuestionItem {
    question: String,
    options: Vec<QuestionOption>,
}

#[derive(Debug, Deserialize)]
struct AskUserInput {
    questions: Vec<QuestionItem>,
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &'static str {
        "AskUserQuestion"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "AskUserQuestion".to_string(),
            description: "Ask the user one or more multiple-choice questions to gather information, clarify ambiguity, or get decisions on implementation choices.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "questions": {
                        "type": "array",
                        "description": "Questions to ask (1-4 questions)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": {
                                    "type": "string",
                                    "description": "The question to ask"
                                },
                                "header": {
                                    "type": "string",
                                    "description": "Short label for the question"
                                },
                                "options": {
                                    "type": "array",
                                    "description": "2-4 answer options",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": { "type": "string" },
                                            "description": { "type": "string" }
                                        },
                                        "required": ["label"]
                                    }
                                }
                            },
                            "required": ["question", "options"]
                        }
                    }
                }),
                required: vec!["questions".to_string()],
            },
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let parsed: AskUserInput = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(id, format!("invalid input: {}", e)),
        };

        let tx_guard = self.tx.lock().await;
        let tx = match tx_guard.as_ref() {
            Some(t) => t.clone(),
            None => return ToolResult::error(id, "no question channel available".to_string()),
        };
        drop(tx_guard);

        let mut answers: Vec<String> = Vec::new();

        for q in &parsed.questions {
            let options: Vec<String> = q.options.iter().map(|o| o.label.clone()).collect();
            let (reply_tx, reply_rx) = oneshot::channel();
            let ask = AskQuestion {
                question: q.question.clone(),
                options,
                reply: reply_tx,
            };
            if tx.send(ask).is_err() {
                return ToolResult::error(id, "failed to send question to user".to_string());
            }
            match reply_rx.await {
                Ok(answer) => answers.push(format!("{}: {}", q.question, answer)),
                Err(_) => answers.push(format!("{}: (no answer)", q.question)),
            }
        }

        ToolResult::success(id, answers.join("\n"))
    }
}
