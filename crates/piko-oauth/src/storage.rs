// Credential storage — mirrors claude-code's platform-aware secure storage.
//
// Priority:
//   macOS   → macOS Keychain (via `security` CLI) with plaintext file fallback
//   Linux   → ~/.config/pikoclaw/.credentials.json  (mode 0o600)
//   Windows → %APPDATA%\pikoclaw\pikoclaw\.credentials.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix timestamp **in milliseconds** when the access token expires.
    pub expires_at_ms: u64,
}

impl StoredTokens {
    pub fn is_expired(&self) -> bool {
        crate::token::is_expired(self.expires_at_ms)
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

pub fn load_tokens() -> Result<Option<StoredTokens>> {
    let raw = load_raw()?;
    raw.map(|s| serde_json::from_str(&s).context("Failed to parse stored credentials"))
        .transpose()
}

pub fn save_tokens(tokens: &StoredTokens) -> Result<()> {
    let json = serde_json::to_string(tokens)?;
    save_raw(&json)
}

pub fn delete_tokens() -> Result<()> {
    delete_raw()
}

// ─── Platform dispatch ───────────────────────────────────────────────────────

fn load_raw() -> Result<Option<String>> {
    #[cfg(target_os = "macos")]
    if let Some(data) = keychain::read() {
        return Ok(Some(data));
    }

    load_from_file()
}

fn save_raw(data: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    match keychain::write(data) {
        Ok(()) => return Ok(()),
        Err(e) => tracing::warn!("Keychain write failed ({e}), falling back to file storage"),
    }

    save_to_file(data)
}

fn delete_raw() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        keychain::delete();
    }
    delete_file()
}

// ─── File storage (all platforms) ────────────────────────────────────────────

fn credentials_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("dev", "pikoclaw", "pikoclaw")
        .map(|d| d.config_dir().join(".credentials.json"))
}

fn load_from_file() -> Result<Option<String>> {
    let path = match credentials_path() {
        Some(p) => p,
        None => return Ok(None),
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn save_to_file(data: &str) -> Result<()> {
    let path =
        credentials_path().ok_or_else(|| anyhow::anyhow!("Cannot determine credentials path"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, data)?;

    // Owner read/write only — matches Claude Code's 0o600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

fn delete_file() -> Result<()> {
    let path = match credentials_path() {
        Some(p) => p,
        None => return Ok(()),
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

// ─── macOS Keychain (via `security` CLI) ─────────────────────────────────────
// Matches claude-code's macOsKeychainStorage approach: calls the `security`
// binary rather than linking against Security.framework, keeping the build
// simple and cross-compilable.

#[cfg(target_os = "macos")]
mod keychain {
    use anyhow::Result;
    use std::process::Command;

    const SERVICE: &str = "pikoclaw-credentials";

    fn username() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "pikoclaw".to_string())
    }

    pub fn read() -> Option<String> {
        let user = username();
        let out = Command::new("security")
            .args(["find-generic-password", "-a", &user, "-w", "-s", SERVICE])
            .output()
            .ok()?;

        if out.status.success() {
            String::from_utf8(out.stdout)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        }
    }

    pub fn write(data: &str) -> Result<()> {
        let user = username();
        // `-U` → update existing entry if present
        let status = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-a",
                &user,
                "-s",
                SERVICE,
                "-w",
                data,
            ])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "`security` CLI returned non-zero status while writing to Keychain"
            ))
        }
    }

    pub fn delete() -> bool {
        let user = username();
        Command::new("security")
            .args(["delete-generic-password", "-a", &user, "-s", SERVICE])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
