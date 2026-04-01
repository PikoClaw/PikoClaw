use std::path::{Path, PathBuf};

const MEMORY_INSTRUCTION_PREFIX: &str =
    "Codebase and user instructions are shown below. Be sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.\n\n";

pub fn load_claude_md(cwd: &Path) -> Option<String> {
    let mut sections: Vec<String> = Vec::new();

    if let Some(global) = load_user_claude_md() {
        sections.push(format!(
            "# User Instructions (~/.claude/CLAUDE.md)\n\n{}",
            global
        ));
    }

    let project_files = find_project_claude_md_files(cwd);
    for (label, content) in project_files {
        sections.push(format!("# {}\n\n{}", label, content));
    }

    if sections.is_empty() {
        return None;
    }

    Some(format!(
        "{}{}",
        MEMORY_INSTRUCTION_PREFIX,
        sections.join("\n\n---\n\n")
    ))
}

fn load_user_claude_md() -> Option<String> {
    let dirs = directories::UserDirs::new()?;
    let home = dirs.home_dir();
    let path = home.join(".claude").join("CLAUDE.md");
    std::fs::read_to_string(&path).ok()
}

fn find_project_claude_md_files(cwd: &Path) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut current = cwd.to_path_buf();

    loop {
        for candidate in project_claude_md_candidates(&current) {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if !content.trim().is_empty() {
                    let label = format!("Project Instructions ({})", candidate.display());
                    results.push((label, content));
                }
            }
        }

        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
                if is_filesystem_root(&current) {
                    break;
                }
            }
            _ => break,
        }
    }

    results.reverse();
    results
}

fn project_claude_md_candidates(dir: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![
        dir.join("CLAUDE.md"),
        dir.join(".claude").join("CLAUDE.md"),
        dir.join("CLAUDE.local.md"),
    ];

    let rules_dir = dir.join(".claude").join("rules");
    if let Ok(entries) = std::fs::read_dir(&rules_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                candidates.push(path);
            }
        }
    }

    candidates
}

fn is_filesystem_root(path: &Path) -> bool {
    path.parent().is_none() || path == Path::new("/")
}
