// Temporary localhost HTTP server that captures the OAuth redirect.
// Mirrors claude-code/services/oauth/auth-code-listener.ts

use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::constants::CLAUDEAI_SUCCESS_URL;

pub struct CallbackServer {
    listener: TcpListener,
    port: u16,
}

impl CallbackServer {
    /// Bind to an OS-assigned port on localhost.
    pub async fn bind() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        Ok(Self { listener, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Wait for the browser to redirect to `http://localhost:<port>/callback?code=...&state=...`.
    ///
    /// Ignores unrelated requests (e.g. favicon.ico) and loops until a valid
    /// `/callback` request is received or an error occurs.
    pub async fn wait_for_code(self, expected_state: &str) -> Result<String> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;

            // Read up to 8 KiB — more than enough for a GET redirect.
            let mut buf = [0u8; 8192];
            let n = tokio::time::timeout(std::time::Duration::from_secs(10), stream.read(&mut buf))
                .await
                .map_err(|_| anyhow!("Timed out reading HTTP request from browser"))??;

            let raw = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();

            // Parse "GET /path?query HTTP/1.1"
            let first_line = raw.lines().next().unwrap_or("");
            let mut parts = first_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");

            if method != "GET" {
                respond(
                    &mut stream,
                    b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n",
                )
                .await;
                continue;
            }

            if !path.starts_with("/callback") {
                respond(
                    &mut stream,
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
                )
                .await;
                continue;
            }

            let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
            let code = query_param(query, "code");
            let state = query_param(query, "state");

            // Validate CSRF state
            if state.as_deref() != Some(expected_state) {
                respond(&mut stream, b"HTTP/1.1 400 Bad Request\r\nContent-Length: 22\r\n\r\nInvalid state parameter").await;
                return Err(anyhow!("OAuth state mismatch — possible CSRF"));
            }

            let code = match code {
                Some(c) => c,
                None => {
                    respond(&mut stream, b"HTTP/1.1 400 Bad Request\r\nContent-Length: 25\r\n\r\nNo authorization code found").await;
                    return Err(anyhow!("No authorization code in OAuth callback"));
                }
            };

            // Redirect browser to success page
            let resp = format!(
                "HTTP/1.1 302 Found\r\nLocation: {CLAUDEAI_SUCCESS_URL}\r\nContent-Length: 0\r\n\r\n"
            );
            respond(&mut stream, resp.as_bytes()).await;

            return Ok(code);
        }
    }
}

/// Parse a single query parameter value (percent-decoded) from a query string.
fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == name {
            Some(percent_decode(v))
        } else {
            None
        }
    })
}

/// Minimal percent-decoding for OAuth code/state values.
pub fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((h1 * 16 + h2) as u8 as char);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

async fn respond(stream: &mut tokio::net::TcpStream, bytes: &[u8]) {
    let _ = stream.write_all(bytes).await;
    let _ = stream.flush().await;
}

/// Extract `code` and validate `state` from a redirect URL or query string
/// pasted by the user in the manual flow.
pub fn extract_code_from_pasted(input: &str, expected_state: &str) -> Result<String> {
    // Accept full URLs or bare query strings
    let query = if let Some(pos) = input.find('?') {
        &input[pos + 1..]
    } else {
        input
    };

    let code = query_param(query, "code");
    let state = query_param(query, "state");

    if let Some(ref s) = state {
        if s != expected_state {
            return Err(anyhow!("State mismatch in pasted URL — please try again"));
        }
    }

    code.ok_or_else(|| anyhow!("Could not find 'code' parameter in the pasted URL"))
}
