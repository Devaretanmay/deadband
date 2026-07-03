---
type: Reference
title: Orchestrator Configuration
description: Rust struct reference for configuring the Loopless orchestrator.
tags: [deadband, configuration, orchestrator]
timestamp: 2026-07-03T00:00:00Z
---

# Orchestrator Configuration

The `OrchestratorConfig` struct controls which detectors are enabled and
their thresholds.

## Rust Struct

```rust
#[derive(Clone, Debug)]
pub struct OrchestratorConfig {
    pub history_size: usize,         // Default: 100
    pub enable_semantic: bool,       // Default: true
    pub enable_rules: bool,          // Default: true
    pub enable_budget: bool,         // Default: true
    pub enable_exact: bool,          // Default: true
    pub error_threshold: u32,        // Default: 1
    pub exact_threshold: u32,        // Default: 1
}
```

## Fields

| Field | Default | Description |
|-------|---------|-------------|
| `history_size` | 100 | Maximum number of past events to keep in memory |
| `enable_semantic` | true | Enable semantic sidecar and history detectors |
| `enable_rules` | true | Enable rule-based validation |
| `enable_budget` | true | Enable call count budget tracking |
| `enable_exact` | true | Enable exact repeat detection |
| `error_threshold` | 1 | Minimum error occurrences before detection |
| `exact_threshold` | 1 | Minimum repeats before detection |

## Adaptive Thresholding

When `ErrorPattern` detections fire, the orchestrator automatically
reduces `exact_threshold` by 1 (minimum 2) to force faster pivots
when agents are stuck in error loops.
