# Spec: Multi-Agent / Coordinator / Swarm

**Status**: 🔶 Partial — `AgentTool` single sub-agents exist; coordinator and swarm not implemented
**TS source**: `coordinator/coordinatorMode.ts`, `tools/TeamCreateTool.tsx`, `tools/SendMessageTool.tsx`

---

## Overview

Beyond single sub-agents (already implemented via `AgentTool`), the TS codebase has a full multi-agent orchestration system:

1. **Coordinator Mode** — a meta-agent that can spin up specialized sub-agents and coordinate their work
2. **Teams** — named groups of agents that can communicate via `SendMessage`
3. **Agent Swarms** — parallel agent execution with shared context

This spec covers what would need to be built to match TS parity.

---

## Current State

- `AgentTool` spawns a single isolated sub-agent and returns its output — this is basic sub-agent support
- No persistent named agents, no inter-agent messaging, no coordinator mode

---

## Part 1: Coordinator Mode

### What it Does

Coordinator mode gives the main agent a special system prompt that instructs it to act as an orchestrator: breaking down complex tasks, delegating to sub-agents, and synthesizing results.

### System Prompt Additions

When coordinator mode is enabled, inject into system prompt:

```
You are a coordinator agent. Your role is to:
1. Break down the user's request into parallel subtasks where possible
2. Spawn specialized sub-agents using the Agent tool for each subtask
3. Monitor their progress using TaskGet/TaskOutput
4. Synthesize their results into a coherent response

Available specialized agent types:
- general-purpose: For research, coding, and general tasks
- Explore: For codebase exploration and search
- Plan: For architecture and implementation planning
```

### Implementation

```toml
[agent]
coordinator_mode = true   # config flag
```

CLI flag: `--coordinator`

When enabled: prepend coordinator system prompt to base system prompt. No new code required — it's just prompt engineering + ensuring Task tools are available.

---

## Part 2: Teams (Named Persistent Agents)

Teams allow creating named agents that persist for the session and can receive messages.

### TeamCreateTool

```
Input: {
    team_name: string,              // unique name for this agent instance
    description: string,            // what this agent specializes in
    system_prompt?: string,         // custom instructions
    tools?: string[],               // allowed tools
}
Output: { team_name: string }
```

### TeamDeleteTool

```
Input: { team_name: string }
Output: string (confirmation)
```

### SendMessageTool

```
Input: {
    to: string,            // team_name or task_id of target agent
    message: string,       // message to send
}
Output: string    // the recipient agent's response
```

### Implementation

```rust
// In piko-agent: persistent named agents as TaskHandle entries
pub struct TeamMember {
    pub name: String,
    pub description: String,
    pub system_prompt: Option<String>,
    pub mailbox: mpsc::Sender<TeamMessage>,    // send messages to this agent
    pub response_rx: mpsc::Receiver<String>,   // receive its replies
}

pub struct TeamRegistry {
    members: HashMap<String, TeamMember>,
}
```

Each team member runs as a background Tokio task with a persistent conversation context. `SendMessage` sends a message to its mailbox and awaits a response.

---

## Part 3: Agent Swarms

Swarms are batches of agents working in parallel on the same task, with results merged.

### Basic Swarm Pattern

The main agent uses `TaskCreate` to launch multiple parallel tasks, then `TaskOutput` to collect results:

```python
# Agent pseudocode
task_ids = []
for subtask in subtasks:
    result = TaskCreate(prompt=subtask)
    task_ids.append(result.task_id)

# Wait for all to complete
while any tasks still running:
    sleep/poll

# Collect results
results = [TaskOutput(id) for id in task_ids]
# Synthesize...
```

This doesn't require new tooling beyond the Task system (see [19_task_system.md](19_task_system.md)).

---

## Part 4: Agent Communication Protocol

For `SendMessage` between agents, messages flow via channels:

```
Main agent → SendMessageTool(to="researcher", message="find X")
           → TeamRegistry lookup → find TeamMember "researcher"
           → send via mailbox channel
           → researcher agent processes, appends to its context
           → researcher responds via response channel
           → SendMessageTool returns response
```

The "researcher" agent runs its own turn in response to the incoming message, using its own system prompt and tool access.

---

## Priority Assessment

| Feature | Priority | Notes |
|---------|----------|-------|
| Coordinator mode | Medium | Mostly prompt engineering, low code |
| Task system (prerequisite) | High | Required for teams and swarms |
| Teams + SendMessage | Low | Complex, limited use cases |
| Swarms | Low | Achievable with Task system |

Recommended order: Task system → Coordinator mode → Teams
