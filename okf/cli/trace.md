---
type: CLI Command
title: deadband trace
description: Reads JSON-lines events from stdin and prints interventions in real-time.
tags: [deadband, cli, trace, events]
timestamp: 2026-07-03T00:00:00Z
---

# deadband trace

Records execution events from stdin and prints interventions in real-time.

## Synopsis

```bash
deadband trace [--config <path>]
```

## Input Format

Reads JSON-lines (one `ToolCallEvent` per line):

```json
{"version":1,"id":"...","timestamp":"...","thread_id":"session-1","step":0,"tool_name":"search","arguments":{"q":"hello"},"status":"started"}
```

## Output

Prints events with any interventions inline:

```
Loopless Trace — reading events from stdin (JSON lines)
Press Ctrl+C to stop

[0] Intervention: abort
```
