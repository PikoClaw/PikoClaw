use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct StdioTransport {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioTransport {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        Ok(Self {
            _child: child,
            stdin,
            stdout,
        })
    }

    pub async fn send(&mut self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut json = serde_json::to_string(request)?;
        json.push('\n');
        self.stdin.write_all(json.as_bytes()).await?;

        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        let response: JsonRpcResponse = serde_json::from_str(line.trim())?;
        Ok(response)
    }
}
