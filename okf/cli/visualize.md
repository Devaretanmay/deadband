---
type: CLI Command
title: deadband visualize
description: Renders an ASCII timeline of a trace for quick visual debugging.
tags: [deadband, cli, visualize, debugging]
timestamp: 2026-07-03T00:00:00Z
---

# deadband visualize

Renders an ASCII timeline of a saved trace.

## Synopsis

```bash
deadband visualize <trace.json>
```

## Example Output

```
Timeline (5 events):

  0 ..........+
  1 ..........+
  2 ..........x !
  3 ..........+
  4 ..........+

Legend: . = started  + = succeeded  x = failed  ! = intervention
```
