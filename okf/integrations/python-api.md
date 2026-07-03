---
type: Integration
title: Python API
description: PyO3-based Python bindings for the Loopless Rust engine.
tags: [deadband, python, bindings, integration]
timestamp: 2026-07-03T00:00:00Z
---

# Python API

Loopless provides Python bindings via PyO3, exposing the full detection
and intervention engine to Python.

## Installation

```bash
pip install deadband
```

## Classes

### Orchestrator

The main entry point for loop detection.

```python
from deadband import Orchestrator

orchestrator = Orchestrator("deadband.yaml")
intervention = orchestrator.process(
    thread_id="session-1",
    step=0,
    tool_name="get_revenue",
    arguments='{"year": 2024}',
)

if intervention:
    if intervention.is_abort():
        print(f"Aborted: {intervention.reason()}")
    elif intervention.is_retry():
        print(f"Retry: {intervention.delay_ms()}ms")
    elif intervention.is_inject_prompt():
        print(f"Prompt: {intervention.prompt_content()}")

metrics = orchestrator.metrics
print(f"Interventions: {metrics.intervention_count()}")
print(f"Calls prevented: {metrics.prevented_calls()}")
```

### Intervention Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `is_continue()` | `bool` | Should execution continue? |
| `is_abort()` | `bool` | Should execution be halted? |
| `is_retry()` | `bool` | Should the call be retried? |
| `is_replace_tool()` | `bool` | Should the tool be replaced? |
| `is_inject_prompt()` | `bool` | Should a prompt be injected? |
| `reason()` | `Optional[str]` | Abort reason |
| `delay_ms()` | `Optional[int]` | Retry delay |
| `prompt_content()` | `Optional[str]` | Prompt injection text |

### RecoveryMetrics Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `intervention_count()` | `int` | Total interventions applied |
| `prevented_calls()` | `int` | Tool calls prevented |
| `recovery_time_ms()` | `int` | Time spent in recovery |
| `to_json()` | `str` | JSON serialization |

### canonicalize_args

Preview what volatile fields get stripped before calling the engine.

```python
from deadband import canonicalize_args

cleaned = canonicalize_args(
    '{"query": "python", "req_id": 42}',
    ["req_id"]
)
# Returns: '{"query":"python"}'
```
