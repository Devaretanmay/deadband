---
type: Component
title: Intervention System
description: The set of actions Loopless can take when a loop is detected, with graceful capability downgrade.
tags: [deadband, intervention, recovery]
timestamp: 2026-07-03T00:00:00Z
---

# Intervention System

The **Intervention System** defines the actions Loopless can take when
a loop is detected. Interventions are extensible and can be downgraded
when a framework adapter lacks the capability to execute them.

## Intervention Types

| Variant | Purpose | Fields |
|---------|---------|--------|
| `Continue` | Allow execution to proceed normally | — |
| `Retry` | Retry the tool call with optional delay | `delay_ms` |
| `Backoff` | Exponential backoff retry | `base_ms`, `attempt` |
| `ReplaceTool` | Replace one tool with another | `original`, `replacement` |
| `InjectPrompt` | Insert guidance into the agent's context | `content`, `position` |
| `Abort` | Halt execution with a reason | `reason` |
| `Custom` | Extensible custom action | `name`, `payload` |

## Prompt Positions

When injecting prompts, the position determines where the prompt is placed:

| Position | Description |
|----------|-------------|
| `BeforeNext` | Injected before the next tool call |
| `ReplaceLast` | Replaces the agent's last response |
| `AfterTool` | Injected after the current tool result |

## AdapterCapabilities

Each framework adapter declares what interventions it supports:

```rust
pub struct AdapterCapabilities {
    pub retry: bool,
    pub replace_tool: bool,
    pub inject_prompt: bool,
    pub abort: bool,
    pub checkpoint_restore: bool,
    pub max_backoff_ms: u64,
}
```

When an adapter doesn't support a requested intervention, the system
downgrades to the closest supported alternative. For example:
`ReplaceTool → InjectPrompt → Retry → Abort → Continue`
