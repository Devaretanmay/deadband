---
type: CLI Command
title: deadband inspect
description: Detailed view of a trace including every event and intervention.
tags: [deadband, cli, inspect, debugging]
timestamp: 2026-07-03T00:00:00Z
---

# deadband inspect

Displays a detailed view of a saved trace.

## Synopsis

```bash
deadband inspect <trace.json>
```

## Example Output

```
Execution ID: a1b2c3d4
Started:      2026-07-03T00:00:00Z
Events:       10
Interventions: 3
Prevented:    14
Recovery:     84ms

Events:
   0. [STARTED] search {"q":"hello"}
   1. [OK]      search {"q":"hello"}
   2. [FAILED]  search {"q":"hello"}
   3. [STARTED] search {"q":"hello"}
   ...

Interventions:
  Event 2: InjectPrompt { content: "Try a different approach", ... }
  Event 3: Abort { reason: "detected loop" }
```
