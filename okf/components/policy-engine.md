---
type: Component
title: Policy Engine
description: Evaluates YAML-defined policies to map detections to interventions.
tags: [deadband, policy, yaml, configuration]
timestamp: 2026-07-03T00:00:00Z
---

# Policy Engine

The **Policy Engine** evaluates user-defined YAML policies to determine
what intervention should be taken when a loop is detected.

## Policy Structure

```yaml
policies:
  - name: "<policy name>"
    when:
      any: [<DetectionKind>, ...]           # Match ANY of these
      # OR
      all: [<DetectionKind>, ...]           # Match ALL of these
      # OR
      count:
        <DetectionKind>: <threshold>        # Match when count >= threshold
    do:
      type: "<ActionType>"
      params:
        <key>: <value>
    priority: <i32>                          # Higher = evaluated first
```

## Condition Types

| Condition | Description |
|-----------|-------------|
| `any` | Match if any listed detection kind is present |
| `all` | Match if all listed detection kinds are present |
| `count` | Match if a detection kind's count meets or exceeds the threshold |

Supported detection kinds:
- `ExactRepeat` — Same tool + same arguments
- `SemanticRepeat` — Same intent, different tool name
- `RuleViolation` — Tool call violated a validation rule
- `ErrorPattern` — Same error type recurring
- `BudgetExceeded` — Call count budget exceeded

## Action Types

| Action | Parameters | Description |
|--------|------------|-------------|
| `Retry` | `delay_ms` | Retry the tool call after a delay |
| `Backoff` | `base_ms` | Exponential backoff (`base * 2^attempt`) |
| `ReplaceTool` | `original`, `replacement` | Replace the tool name |
| `InjectPrompt` | `content`, `position` | Inject guidance into the agent's context |
| `Abort` | `reason` | Stop execution entirely |

## Capability Downgrade

When an adapter doesn't support a requested intervention, the
`AdapterCapabilities` system automatically downgrades to the closest
supported fallback:

```
InjectPrompt → Retry → Abort → Continue
ReplaceTool → InjectPrompt → Retry → Abort → Continue
```
