// Cross-platform browser opener — mirrors claude-code/utils/browser.ts

use std::process::Command;

/// Open `url` in the user's default browser.
/// Returns `true` if the OS command succeeded, `false` otherwise.
/// Does not fail the overall auth flow on error — the manual URL fallback
/// is always printed before this is called.
pub async fn open(url: &str) -> bool {
    // Validate protocol
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    let url = url.to_string();
    tokio::task::spawn_blocking(move || open_sync(&url))
        .await
        .unwrap_or(false)
}

fn open_sync(url: &str) -> bool {
    // Respect BROWSER env var override (same as claude-code)
    if let Ok(browser) = std::env::var("BROWSER") {
        return Command::new(&browser)
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    {
        // rundll32 url.dll,OpenURL <url>  (matches claude-code)
        Command::new("rundll32")
            .args(["url.dll,OpenURL", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux and other Unix-likes
        Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
