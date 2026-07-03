---
type: Feature
title: Semantic Sidecar Shadow Mode
description: When the semantic sidecar is unreachable, Loopless gracefully degrades with logging and metrics.
tags: [deadband, detection, semantic, sidecar, shadow-mode]
timestamp: 2026-07-03T00:00:00Z
---

# Shadow Mode

When the optional **semantic sidecar** (BERT embedding server) is
unreachable, Loopless enters **shadow mode** — a graceful degradation
state that preserves functionality while tracking the impact.

## Behavior

| Event | What Happens |
|-------|--------------|
| Sidecar unreachable | `tracing::warn!` logged once |
| Sidecar returns error | `tracing::warn!` logged, shadow mode entered |
| Sidecar recovers | `tracing::info!` logged, shadow mode exited |
| Detection during shadow | Exact detection only (semantic skipped) |

## Metrics

Shadow metrics are tracked via `SidecarShadowMetrics`:

```rust
pub struct SidecarShadowMetrics {
    pub sidecar_unavailable_count: u64,   // How many times sidecar was down
    pub shadow_loops_missed: u64,         // Loops caught during shadow
    pub shadow_mode_active: bool,         // Currently in shadow mode?
}
```

## Querying Shadow Status

```rust
let detector = SemanticDetector::new(sidecar);
let metrics = detector.shadow_metrics();
println!("Shadow mode: {}", metrics.shadow_mode_active);
println!("Unavailable count: {}", metrics.sidecar_unavailable_count);
```

## Rationale

Shadow mode gives users confidence to enable the semantic sidecar without
disruption. They can see exactly how many loops they're missing while the
sidecar is down, making the decision to run the 100MB embedding model
data-driven rather than speculative.
