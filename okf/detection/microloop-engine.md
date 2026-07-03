---
type: Component
title: Microloop Detection Engine
description: The imported Microloop crate provides 460ns loop detection via HistoryTracker, RuleEngine, and canonicalization.
tags: [deadband, microloop, detection, engine]
timestamp: 2026-07-03T00:00:00Z
---

# Microloop Detection Engine

Loopless imports [Microloop](https://github.com/Devaretanmay/microloop)
as its detection backend. This provides battle-tested, sub-microsecond
loop detection without duplicating detection logic.

## Dependency

```toml
[dependencies]
microloop = { path = "../microloop" }
```

## Integrated Components

| Microloop Component | Used By | Purpose |
|---------------------|---------|---------|
| `HistoryTracker` | `ExactDetector` | 460ns exact repeat counting |
| `RuleEngine` / `CompiledRule` | `RuleDetector` | Regex, exact, JSON schema validation |
| `canonical::strip_volatile_fields` | `ExactDetector` + Python | Dot-notation field stripping |
| `auto_infer_volatile_fields` | `ExactDetector` | High-entropy field detection |

## Detector Architecture

| Detector | Backend | Detection Type |
|----------|---------|----------------|
| `ExactDetector` | `microloop::HistoryTracker` + `canonical` | `ExactRepeat` |
| `SemanticDetector` | `SemanticSidecarClient` (native) | `SemanticRepeat` |
| `RuleDetector` | `microloop::engine::CompiledRule` | `RuleViolation` |
| `HistoryDetector` | Native | `ErrorPattern` |
| `BudgetDetector` | Native | `BudgetExceeded` |

## Performance

| Check | Latency |
|-------|---------|
| Exact repeat detection | ~460ns |
| Full pipeline (5 detectors) | < 5ms |
