# Spec: Session List & Management Commands

**Status**: ❌ Todo
**Rust crate**: `piko-tui`, `piko-session`, `piko-skills`
**TS source**: `commands/session.ts`, `screens/ResumeConversation.tsx`

---

## Overview

Commands for viewing, switching, naming, and deleting sessions from within the TUI. Currently, session management is only possible via CLI subcommands before launch (`continue`, `resume`). These commands add in-session management.

---

## Commands to Implement

### `/sessions`

List all saved sessions:

```
Sessions:
  abc12345  [current]  main project     2025-01-15 14:32  /home/user/myproject
  def67890             auth refactor    2025-01-14 09:15  /home/user/myproject
  ghi11111             quick question   2025-01-13 22:01  /tmp
  jkl22222             untitled         2025-01-12 18:44  /home/user/other

Type /resume <id> to switch sessions.
```

Columns:
- ID (short, first 8 chars of UUID)
- `[current]` marker for active session
- Display name (from `session.name` or truncated first user message)
- Last updated timestamp
- Working directory

### `/resume <id>`

Switch to a different session from within TUI:

```
/resume def67890
```

1. Save current session state
2. Load target session
3. Replace `ConversationContext` with loaded session's messages
4. Update TUI to show loaded session's cwd in status bar
5. Show confirmation: `Resumed session: auth refactor`

### `/delete <id>`

Delete a session:

```
/delete def67890
→ Delete session "auth refactor"? [y/n]
→ Session deleted.
```

- Prompts for confirmation before deleting
- Cannot delete currently active session (show error)
- Removes session JSON file and updates index

### `/rename <name>`

Rename the current session:

```
/rename "auth refactor - completed"
→ Session renamed.
```

Updates `session.name`, saves immediately.

---

## Implementation Plan

### Step 1: Load All Sessions for Listing

```rust
// In piko-session/store.rs
impl FilesystemSessionStore {
    pub async fn list_all(&self) -> Result<Vec<SessionInfo>> {
        // scan sessions directory
        // read each {uuid}.json
        // return Vec<SessionInfo> sorted by updated_at desc
    }
}

pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cwd: PathBuf,
    pub message_count: usize,
    pub display_name: String,  // name or truncated first user message
}
```

### Step 2: `/sessions` Command

In `piko-skills/built_ins`:

```rust
pub async fn handle_sessions(context: &mut CommandContext) -> DispatchResult {
    let sessions = context.session_store.list_all().await?;

    let output = sessions.iter().enumerate().map(|(i, s)| {
        let current = if s.id == context.current_session_id { " [current]" } else { "" };
        format!(
            "  {}{}  {:<20}  {}  {}",
            &s.id[..8],
            current,
            s.display_name,
            s.updated_at.format("%Y-%m-%d %H:%M"),
            s.cwd.display()
        )
    }).collect::<Vec<_>>().join("\n");

    DispatchResult::ShowText(format!("Sessions:\n{output}\n\nType /resume <id> to switch."))
}
```

### Step 3: `/resume` Command (in-TUI)

```rust
pub async fn handle_resume(id_prefix: &str, context: &mut CommandContext) -> DispatchResult {
    // find session by ID prefix
    let sessions = context.session_store.list_all().await?;
    let session = sessions.iter().find(|s| s.id.starts_with(id_prefix))
        .ok_or_else(|| format!("Session not found: {id_prefix}"))?;

    // save current session first
    context.session_store.save(&context.current_session).await?;

    // load target session
    let loaded = context.session_store.load(&session.id).await?;
    context.replace_conversation(loaded.messages);
    context.current_session_id = session.id.clone();

    DispatchResult::ShowText(format!("Resumed: {}", session.display_name))
}
```

### Step 4: `/delete` Command

```rust
pub async fn handle_delete(id_prefix: &str, context: &mut CommandContext) -> DispatchResult {
    if id_prefix == &context.current_session_id[..id_prefix.len()] {
        return DispatchResult::Error("Cannot delete current session".into());
    }
    // find + confirm + delete
    // context.session_store.delete(&session.id).await?
}
```

Need a new `FilesystemSessionStore::delete(id)` method:
```rust
pub async fn delete(&self, id: &str) -> Result<()> {
    let path = self.session_path(id);
    tokio::fs::remove_file(path).await?;
    self.update_index_remove(id).await?;
    Ok(())
}
```

### Step 5: `/rename` Command

```rust
pub async fn handle_rename(name: &str, context: &mut CommandContext) -> DispatchResult {
    context.current_session.name = Some(name.to_string());
    context.session_store.save(&context.current_session).await?;
    DispatchResult::ShowText(format!("Session renamed to: {name}"))
}
```

---

## TUI Integration

The `CommandContext` struct needs access to:
- `session_store: Arc<FilesystemSessionStore>`
- `current_session_id: String`
- `replace_conversation(messages: Vec<Message>)` method

Pass these into the `SkillDispatcher` via the `ToolContext` or a new `CommandContext` struct.
