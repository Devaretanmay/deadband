---
type: CLI Command
title: deadband doctor
description: Health check that validates configuration, initializes the engine, and checks the semantic sidecar.
tags: [deadband, cli, doctor, diagnostics]
timestamp: 2026-07-03T00:00:00Z
---

# deadband doctor

Validates your Loopless setup.

## Synopsis

```bash
deadband doctor [--config <path>]
```

## Checks Performed

1. **Config** — Reads and validates the YAML policy file
2. **Core** — Initializes the Orchestrator with the config
3. **Sidecar** — Checks if the semantic sidecar (localhost:8081) is running

## Example Output

```
Loopless Doctor
===============
  Config: OK (deadband.yaml loaded)
  Core:   OK (5 policies, 4 detectors)
  Sidecar: WARN (not running — semantic detection disabled)
```
