use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use reqwest::Client;
use serde::Deserialize;

pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("PikoClaw/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct WebFetchInput {
    url: String,
    #[serde(default)]
    max_length: Option<usize>,
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "WebFetch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "WebFetch".to_string(),
            description: "Fetches content from a URL and returns it as plain text. HTML is converted to readable text.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "Maximum characters to return (default: 50000)"
                    }
                }),
                required: vec!["url".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let parsed: WebFetchInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => return ToolResult::error("", format!("invalid input: {}", e)),
        };

        let tool_use_id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let max_length = parsed.max_length.unwrap_or(50_000);

        let resp = match self.client.get(&parsed.url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(tool_use_id, format!("fetch failed: {}", e)),
        };

        let status = resp.status();
        if !status.is_success() {
            return ToolResult::error(tool_use_id, format!("HTTP {}", status));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(tool_use_id, format!("failed to read body: {}", e)),
        };

        let text = if content_type.contains("text/html") {
            html_to_text(&body)
        } else {
            body
        };

        let truncated = if text.len() > max_length {
            format!(
                "{}\n...(truncated at {} chars)",
                &text[..max_length],
                max_length
            )
        } else {
            text
        };

        ToolResult::success(tool_use_id, truncated)
    }

    fn is_read_only(&self) -> bool {
        true
    }
}

fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut tag_buf = String::new();

    let bytes = html.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_tag {
            tag_buf.push(ch);
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_buf.to_lowercase();
                if tag_lower.starts_with("script") || tag_lower.starts_with("/script") {
                    in_script = tag_lower.starts_with("script");
                } else if tag_lower.starts_with("style") || tag_lower.starts_with("/style") {
                    in_style = tag_lower.starts_with("style");
                } else if tag_lower.starts_with("br")
                    || tag_lower.starts_with("p")
                    || tag_lower.starts_with("/p")
                    || tag_lower.starts_with("div")
                    || tag_lower.starts_with("/div")
                {
                    result.push('\n');
                }
                tag_buf.clear();
            }
        } else if ch == '<' {
            in_tag = true;
            tag_buf.clear();
        } else if !in_script && !in_style {
            if ch == '&' {
                let rest = &html[i..];
                if rest.starts_with("&amp;") {
                    result.push('&');
                    i += 4;
                } else if rest.starts_with("&lt;") {
                    result.push('<');
                    i += 3;
                } else if rest.starts_with("&gt;") {
                    result.push('>');
                    i += 3;
                } else if rest.starts_with("&nbsp;") {
                    result.push(' ');
                    i += 5;
                } else if rest.starts_with("&quot;") {
                    result.push('"');
                    i += 5;
                } else {
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        }
        i += 1;
    }

    let cleaned: String = result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    cleaned
}
