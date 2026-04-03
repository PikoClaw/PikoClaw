# Spec: Cron Scheduler & Remote Triggers

**Status**: ❌ Todo
**TS source**: `utils/cron.ts`, `utils/cronTasks.ts`, `tools/CronCreateTool.tsx`, `tools/RemoteTriggerTool.tsx`

---

## Overview

The cron scheduler allows agents to schedule recurring or one-shot tasks that run at specific times or intervals. Remote triggers allow triggering remote agent sessions from the local PikoClaw instance.

---

## CronCreateTool

```
Input: {
    schedule: string,     // cron expression: "0 9 * * 1-5" (weekdays 9am)
                          // or interval: "every 5m", "every 1h"
    prompt: string,       // what the agent should do when triggered
    name?: string,        // human label for the cron job
    enabled?: bool,       // default: true
}
Output: {
    cron_id: string
}
```

### Cron Expression Format

Standard 5-field cron:
```
┌──── minute (0-59)
│ ┌── hour (0-23)
│ │ ┌─ day of month (1-31)
│ │ │ ┌ month (1-12)
│ │ │ │ ┌ day of week (0-7, Sun=0 or 7)
│ │ │ │ │
* * * * *
```

Also support shorthand:
- `@hourly` = `0 * * * *`
- `@daily` = `0 0 * * *`
- `@weekly` = `0 0 * * 0`
- `every 5m` = every 5 minutes
- `every 1h` = every hour

## CronDeleteTool

```
Input: { cron_id: string }
Output: string (confirmation)
```

## CronListTool

```
Input: {}
Output: CronJob[]
// CronJob: { id, name, schedule, prompt, enabled, last_run?, next_run? }
```

---

## Cron Storage

Cron jobs are persisted to `~/.config/pikoclaw/crons.toml`:

```toml
[[crons]]
id = "abc123"
name = "morning-standup"
schedule = "0 9 * * 1-5"
prompt = "Check for any new GitHub issues assigned to me and summarize them"
enabled = true
last_run = "2025-01-15T09:00:00Z"
```

---

## Implementation Plan

### Step 1: Cron Parser

Use `cron` crate for parsing and next-run calculation:

```toml
[dependencies]
cron = "0.12"
```

```rust
pub fn next_run(schedule: &str) -> Result<DateTime<Utc>> {
    let schedule: Schedule = schedule.parse()?;
    schedule.upcoming(Utc).next()
        .ok_or_else(|| Error::msg("No upcoming run time"))
}
```

### Step 2: Cron Daemon

Background Tokio task that checks every minute:

```rust
pub async fn run_cron_daemon(
    crons_file: PathBuf,
    agent_factory: Arc<dyn AgentFactory>,
) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let now = Utc::now();

        let mut crons = load_crons(&crons_file).await?;
        for cron in &mut crons {
            if cron.enabled && should_run(cron, now) {
                // spawn agent with cron.prompt
                spawn_cron_agent(&cron.prompt, &agent_factory).await;
                cron.last_run = Some(now);
            }
        }
        save_crons(&crons_file, &crons).await?;
    }
}
```

### Step 3: Cron Agent Output

Cron agents run non-interactively (no TUI). Their output:
- Written to `~/.local/share/pikoclaw/cron-logs/{cron_id}/{timestamp}.log`
- Optionally displayed as a notification in TUI if it's running

### Step 4: Tool Implementation

```rust
pub struct CronCreateTool {
    crons_file: PathBuf,
}

impl Tool for CronCreateTool {
    async fn execute(&self, input: CronCreateInput, _: &ToolContext) -> ToolResult {
        let id = Uuid::new_v4().to_string();
        let cron = CronJob {
            id: id.clone(),
            name: input.name,
            schedule: input.schedule,
            prompt: input.prompt,
            enabled: input.enabled.unwrap_or(true),
            last_run: None,
        };
        append_cron(&self.crons_file, &cron).await?;
        ToolResult::success_json(json!({ "cron_id": id }))
    }
}
```

---

## RemoteTriggerTool

The TS `RemoteTriggerTool` triggers agents on Anthropic's remote infrastructure (CCR). Since we don't have access to that, we implement a local equivalent:

**Local Remote Trigger**: Triggers a named agent session that runs in a background process and can be accessed from other terminal windows.

```
Input: {
    trigger_name: string,   // name of the trigger/agent to invoke
    prompt: string,         // what to tell the agent
    wait: bool,             // wait for result (default: false)
}
Output: {
    trigger_id: string,
    result?: string         // if wait: true
}
```

Implementation: write trigger request to a socket file / named pipe; background daemon picks it up and runs.

---

## Priority

Low. Useful for power users who want scheduled automations. Implement after hooks and memory system.
