---
type: Component
title: Orchestrator
description: The central coordinator that processes events, runs detection, evaluates policies, and returns interventions.
tags: [deadband, orchestrator, core]
timestamp: 2026-07-03T00:00:00Z
---

# Orchestrator

The **Orchestrator** is the central coordinator of the Loopless runtime.
It ties together event processing, detection, and policy evaluation.

## Responsibilities

1. **Receive** `ToolCallEvent` from framework adapters
2. **Store** events in a bounded history store (default: 100 entries)
3. **Run** the observation pipeline (all enabled detectors)
4. **Evaluate** policies against any detected issues
5. **Return** an `Intervention` to the caller
6. **Track** metrics for every intervention

## Configuration

See [Orchestrator Config](/configuration/orchestrator-config.md) for the
full configuration reference.

## Adaptive Thresholding

When `ErrorPattern` detections are present, the orchestrator automatically
tightens the `exact_threshold` by 1 — forcing agents to pivot faster when
they're stuck in a failing pattern. This mirrors the behavior of the
Microloop proxy.

## API

```rust
pub struct Orchestrator {
    config: OrchestratorConfig,
    pipeline: ObservationPipeline,
    policy_engine: PolicyEngine,
    history_store: Box<dyn HistoryStore>,
    metrics: RecoveryMetrics,
}

impl Orchestrator {
    pub fn process(
        &mut self,
        event: ToolCallEvent,
        capabilities: &AdapterCapabilities,
    ) -> (Option<Intervention>, Option<DetectionReport>);
}
```

## Metrics

The orchestrator collects:

- `intervention_count` — Number of interventions applied
- `prevented_calls` — Tool calls prevented by interventions
- `recovery_time_ms` — Time spent in recovery
- `detection_breakdown` — Per-detection-kind counter
