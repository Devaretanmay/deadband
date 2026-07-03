---
type: CLI Command
title: deadband init
description: Generates a default deadband.yaml configuration file.
tags: [deadband, cli, init, configuration]
timestamp: 2026-07-03T00:00:00Z
---

# deadband init

Generates a default `deadband.yaml` policy file in the current directory.

## Synopsis

```bash
deadband init [--output <path>]
```

## Generated Configuration

Creates a YAML file with example policies for common loop scenarios:
repeat abort, timeout retry, semantic nudging, budget limits, and
rate-limit backoff.

## Behavior

- **Does not** overwrite an existing file (exits with error)
- Output defaults to `deadband.yaml` in the current directory
