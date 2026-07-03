---
type: Reference
title: Policy YAML Format
description: Complete reference for the Loopless policy YAML schema.
tags: [deadband, configuration, yaml, policy]
timestamp: 2026-07-03T00:00:00Z
---

# Policy YAML Format

Loopless uses YAML to define policies that map detections to interventions.

## Full Schema

```yaml
policies:
  - name: "<string>"                    # Required. Unique policy name
    when:                                # Required. Condition block
      any: ["<DetectionKind>", ...]      # Match if ANY detection kind present
      # OR
      all: ["<DetectionKind>", ...]      # Match if ALL detection kinds present
      # OR
      count:
        "<DetectionKind>": <uint>        # Match if count >= threshold
    do:                                  # Required. Action block
      type: "<ActionType>"               # Required. One of: Retry, Backoff,
                                         #   ReplaceTool, InjectPrompt, Abort
      params:                            # Optional. Action parameters
        <key>: <value>
    priority: <int>                      # Optional. Higher = evaluated first
```

## Detection Kinds

| Kind | Detector | Description |
|------|----------|-------------|
| `ExactRepeat` | ExactDetector | Same tool + same arguments (after volatile strip) |
| `SemanticRepeat` | SemanticDetector | Same intent detected by embedding sidecar |
| `RuleViolation` | RuleDetector | Tool call violates a validation rule |
| `ErrorPattern` | HistoryDetector | Same error type recurring |
| `BudgetExceeded` | BudgetDetector | Call count exceeds budget |

## Action Types

| Action | Params | Description |
|--------|--------|-------------|
| `Retry` | `delay_ms` (u64) | Retry with optional delay |
| `Backoff` | `base_ms` (u64) | Exponential backoff: `base * 2^attempt` |
| `ReplaceTool` | `original` (str), `replacement` (str) | Swap tool name |
| `InjectPrompt` | `content` (str), `position` (str): `BeforeNext`, `ReplaceLast`, or `AfterTool` | Inject guidance |
| `Abort` | `reason` (str) | Halt execution |

## Complete Example

```yaml
policies:
  - name: "hard_fork_on_repeats"
    when:
      count:
        ExactRepeat: 3
    do:
      type: "InjectPrompt"
      params:
        content: "You've tried this tool 3 times. Try a different approach."
        position: "ReplaceLast"

  - name: "retry_timeout"
    when:
      count:
        ErrorPattern: 2
    do:
      type: "Retry"
      params:
        delay_ms: 100

  - name: "semantic_nudge"
    when:
      any:
        - SemanticRepeat
    do:
      type: "InjectPrompt"
      params:
        content: "You're trying similar approaches. Consider a different strategy."
        position: "BeforeNext"

  - name: "abort_on_budget"
    when:
      count:
        ExactRepeat: 10
    do:
      type: "Abort"
      params:
        reason: "Execution exceeded maximum repeat threshold of 10"

  - name: "backoff_on_rate_limit"
    when:
      count:
        ErrorPattern: 1
    do:
      type: "Backoff"
      params:
        base_ms: 500
```
