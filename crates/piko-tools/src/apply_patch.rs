// ApplyPatch tool: apply a unified diff patch to files.
//
// Parses standard unified diff format (as produced by `git diff` or `diff -u`).
// Supports dry_run mode which validates the patch without writing any files.

use crate::tool_trait::{Tool, ToolContext};
use async_trait::async_trait;
use piko_types::tool::{ToolDefinition, ToolInputSchema, ToolResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::debug;

pub struct ApplyPatchTool;

#[derive(Debug, Deserialize)]
struct ApplyPatchInput {
    patch: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug)]
struct Hunk {
    orig_start: usize,
    lines: Vec<(char, String)>,
}

#[derive(Debug)]
struct FilePatch {
    path: String,
    hunks: Vec<Hunk>,
}

fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}

fn parse_unified_diff(patch: &str) -> Result<Vec<FilePatch>, String> {
    let mut file_patches: Vec<FilePatch> = Vec::new();
    let mut current_file: Option<FilePatch> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in patch.lines() {
        if line.starts_with("--- ") {
            if let Some(h) = current_hunk.take() {
                if let Some(ref mut f) = current_file {
                    f.hunks.push(h);
                }
            }
            if let Some(f) = current_file.take() {
                file_patches.push(f);
            }
        } else if let Some(raw) = line.strip_prefix("+++ ") {
            let path = raw.trim_start_matches("b/").trim().to_string();
            current_file = Some(FilePatch {
                path,
                hunks: Vec::new(),
            });
        } else if line.starts_with("@@ ") {
            if let Some(h) = current_hunk.take() {
                if let Some(ref mut f) = current_file {
                    f.hunks.push(h);
                }
            }
            let orig_start = parse_hunk_header(line)?;
            current_hunk = Some(Hunk {
                orig_start,
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            if let Some(s) = line.strip_prefix('+') {
                hunk.lines.push(('+', s.to_string()));
            } else if let Some(s) = line.strip_prefix('-') {
                hunk.lines.push(('-', s.to_string()));
            } else if let Some(s) = line.strip_prefix(' ') {
                hunk.lines.push((' ', s.to_string()));
            }
            // '\\' (no newline at end of file) — ignore
        }
    }

    if let Some(h) = current_hunk.take() {
        if let Some(ref mut f) = current_file {
            f.hunks.push(h);
        }
    }
    if let Some(f) = current_file.take() {
        file_patches.push(f);
    }

    Ok(file_patches)
}

fn parse_hunk_header(line: &str) -> Result<usize, String> {
    let after_at = line
        .strip_prefix("@@ ")
        .ok_or_else(|| format!("Invalid hunk header: {}", line))?;

    let minus_part = after_at
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("Malformed hunk header: {}", line))?;

    let digits = minus_part
        .trim_start_matches('-')
        .split(',')
        .next()
        .unwrap_or("0");

    let line_num: usize = digits
        .parse()
        .map_err(|_| format!("Could not parse line number from: {}", minus_part))?;

    Ok(if line_num > 0 { line_num - 1 } else { 0 })
}

fn apply_hunk(lines: Vec<String>, hunk: &Hunk) -> Result<Vec<String>, String> {
    let expected: Vec<&str> = hunk
        .lines
        .iter()
        .filter(|(c, _)| *c == ' ' || *c == '-')
        .map(|(_, l)| l.as_str())
        .collect();

    let search_start = hunk.orig_start.min(lines.len());
    let pos = find_context_position(&lines, &expected, search_start).ok_or_else(|| {
        format!(
            "Context not found near line {} (looking for {} lines of context/removes)",
            hunk.orig_start + 1,
            expected.len()
        )
    })?;

    let mut output_prefix = lines[..pos].to_vec();
    let mut output_suffix = lines[pos..].to_vec();

    let consume = expected.len();
    if consume > output_suffix.len() {
        return Err(format!(
            "Hunk extends beyond end of file at line {}",
            pos + 1
        ));
    }
    let remaining = output_suffix.split_off(consume);

    let mut replacement: Vec<String> = Vec::new();
    for (ch, content) in &hunk.lines {
        match ch {
            '+' | ' ' => replacement.push(content.clone()),
            '-' => {}
            _ => {}
        }
    }

    output_prefix.append(&mut replacement);
    output_prefix.extend(remaining);
    Ok(output_prefix)
}

fn find_context_position(lines: &[String], expected: &[&str], hint: usize) -> Option<usize> {
    if expected.is_empty() {
        return Some(hint.min(lines.len()));
    }

    let n = lines.len();
    let max_start = if n >= expected.len() {
        n - expected.len()
    } else {
        return None;
    };

    let candidates: Vec<usize> = std::iter::once(hint).chain(0..=max_start).collect();

    for &start in &candidates {
        if start > max_start {
            continue;
        }
        if lines[start..start + expected.len()]
            .iter()
            .zip(expected.iter())
            .all(|(l, e)| l.as_str() == *e)
        {
            return Some(start);
        }
    }
    None
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "ApplyPatch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ApplyPatch".to_string(),
            description: "Apply a unified diff patch to files. The patch must be in standard \
                unified diff format (as produced by `git diff` or `diff -u`). \
                Set dry_run=true to validate the patch without writing any changes."
                .to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::json!({
                    "patch": {
                        "type": "string",
                        "description": "Unified diff patch content"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, validate the patch without applying it (default: false)"
                    }
                }),
                required: vec!["patch".to_string()],
            },
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let id = input
            .get("__tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let params: ApplyPatchInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(id, format!("Invalid input: {}", e)),
        };

        if params.patch.trim().is_empty() {
            return ToolResult::error(id, "patch must not be empty");
        }

        let file_patches = match parse_unified_diff(&params.patch) {
            Ok(fp) => fp,
            Err(e) => return ToolResult::error(id, format!("Failed to parse patch: {}", e)),
        };

        if file_patches.is_empty() {
            return ToolResult::error(
                id,
                "No file diffs found in patch (expected --- / +++ headers)",
            );
        }

        let mut total_added: i64 = 0;
        let mut total_removed: i64 = 0;
        let mut to_write: Vec<(PathBuf, String)> = Vec::new();

        for fp in &file_patches {
            let path = resolve_path(&ctx.cwd, &fp.path);
            debug!(path = %path.display(), "ApplyPatch processing file");

            let original_content = if path.exists() {
                match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(e) => {
                        return ToolResult::error(
                            id,
                            format!("Cannot read {}: {}", path.display(), e),
                        )
                    }
                }
            } else {
                String::new()
            };

            let mut lines: Vec<String> = original_content.lines().map(|l| l.to_string()).collect();

            for (hunk_idx, hunk) in fp.hunks.iter().enumerate() {
                let added = hunk.lines.iter().filter(|(c, _)| *c == '+').count() as i64;
                let removed = hunk.lines.iter().filter(|(c, _)| *c == '-').count() as i64;

                lines = match apply_hunk(lines, hunk) {
                    Ok(l) => l,
                    Err(e) => {
                        return ToolResult::error(
                            id,
                            format!(
                                "Failed to apply hunk {} in {}: {}",
                                hunk_idx + 1,
                                fp.path,
                                e
                            ),
                        )
                    }
                };

                total_added += added;
                total_removed += removed;
            }

            let new_content = if lines.is_empty() {
                String::new()
            } else {
                let mut s = lines.join("\n");
                if original_content.ends_with('\n') || original_content.is_empty() {
                    s.push('\n');
                }
                s
            };

            to_write.push((path, new_content));
        }

        if params.dry_run {
            return ToolResult::success(
                id,
                format!(
                    "Dry run: patch would modify {} file(s) (+{} -{} lines).",
                    to_write.len(),
                    total_added,
                    total_removed,
                ),
            );
        }

        for (path, new_content) in &to_write {
            if let Err(e) = tokio::fs::write(path, new_content).await {
                return ToolResult::error(id, format!("Failed to write {}: {}", path.display(), e));
            }
        }

        ToolResult::success(
            id,
            format!(
                "Applied patch to {} file(s) (+{} -{} lines).",
                to_write.len(),
                total_added,
                total_removed,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -12,5 +12,6 @@").unwrap(), 11);
        assert_eq!(parse_hunk_header("@@ -1,3 +1,4 @@ fn foo()").unwrap(), 0);
        assert_eq!(parse_hunk_header("@@ -0,0 +1 @@").unwrap(), 0);
    }

    #[test]
    fn test_apply_hunk_simple() {
        let lines: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let hunk = Hunk {
            orig_start: 1,
            lines: vec![(' ', "b".into()), ('-', "c".into()), ('+', "C".into())],
        };
        let result = apply_hunk(lines, &hunk).unwrap();
        assert_eq!(result, vec!["a", "b", "C"]);
    }

    #[test]
    fn test_apply_hunk_context_mismatch() {
        let lines: Vec<String> = vec!["x".into(), "y".into()];
        let hunk = Hunk {
            orig_start: 0,
            lines: vec![('-', "z".into())],
        };
        assert!(apply_hunk(lines, &hunk).is_err());
    }

    #[test]
    fn test_parse_unified_diff_basic() {
        let patch = "\
--- a/foo.txt
+++ b/foo.txt
@@ -1,2 +1,2 @@
 hello
-world
+rust
";
        let fps = parse_unified_diff(patch).unwrap();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].path, "foo.txt");
        assert_eq!(fps[0].hunks.len(), 1);
    }
}
