---
type: Component
title: Replay System
description: Serializes execution traces to JSON for debugging and comparison.
tags: [deadband, replay, trace, debugging]
timestamp: 2026-07-03T00:00:00Z
---

# Replay System

The **Replay System** serializes full execution traces to JSON, enabling
offline debugging, trace comparison, and post-mortem analysis.

## Trace Format

```json
{
  "version": 1,
  "execution_id": "uuid-v4",
  "schema_version": 1,
  "started_at": "2026-07-03T00:00:00Z",
  "events": [
    {
      "version": 1,
      "id": "uuid-v4",
      "timestamp": "2026-07-03T00:00:00Z",
      "thread_id": "session-1",
      "step": 0,
      "tool_name": "get_revenue",
      "arguments": {"year": 2024},
      "status": "started"
    }
  ],
  "interventions": [
    {
      "event_index": 2,
      "event": { ... },
      "report": { ... },
      "intervention": {"type": "Abort", "reason": "detected loop"}
    }
  ],
  "metrics": { ... },
  "policy_config": "..."
}
```

## Replayer

The `Replayer` provides:

| Method | Description |
|--------|-------------|
| `from_json(path)` | Load a trace from a JSON file |
| `to_json(trace, path)` | Save a trace to a JSON file |
| `validate(trace)` | Validate trace structure and version |
| `compare(original, replayed)` | Diff two traces for discrepancies |

## CLI Integration

```bash
deadband trace < trace.jsonl     # Record execution
deadband replay trace.json       # Replay a saved trace
deadband inspect trace.json      # Detailed view
deadband visualize trace.json    # ASCII timeline
```
