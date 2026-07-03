---
type: CLI Command
title: deadband replay
description: Replays a saved trace file for debugging.
tags: [deadband, cli, replay, debugging]
timestamp: 2026-07-03T00:00:00Z
---

# deadband replay

Replays a saved execution trace from a JSON file.

## Synopsis

```bash
deadband replay <trace.json>
```

## Example Output

```
Trace: a1b2c3d4 (10 events, 3 interventions)
  Started:  2026-07-03T00:00:00Z
  Loops prevented: 14
  Recovery time: 84ms
```
