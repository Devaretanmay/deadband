---
type: Concept
title: System Architecture
description: The layered architecture of Loopless — adapters, orchestrator, policy engine, and Microloop-powered detection.
tags: [deadband, architecture, components, design]
timestamp: 2026-07-03T00:00:00Z
---

# System Architecture

Loopless uses a layered architecture where each layer has a clear
responsibility. The separation boundary is:

    MICROLOOP = "What happened?"    (detection)
    LOOPLESS  = "What should we do?" (decision + intervention)

## Architecture Diagram

```
┌──────────────────────────────────────────────┐
│                  AGENT                        │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│           FRAMEWORK ADAPTERS                  │
│  LangGraph │ CrewAI │ OpenAI Agents           │
│  Converts framework events → ToolCallEvent    │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│         LOOPLESS ORCHESTRATOR                 │
│  Receives ToolCallEvent                       │
│  Runs detection pipeline                      │
│  Evaluates policies                           │
│  Returns Intervention                         │
│                                               │
│  ┌────────────────────────────────────────┐   │
│  │      OBSERVATION PIPELINE              │   │
│  │  ExactDetector → SemanticDetector      │   │
│  │  RuleDetector → HistoryDetector        │   │
│  │  BudgetDetector                        │   │
│  └────────────────────────────────────────┘   │
│                                               │
│  ┌────────────────────────────────────────┐   │
│  │         POLICY ENGINE                  │   │
│  │  YAML conditions → Intervention        │   │
│  │  AdapterCapabilities.downgrade()       │   │
│  └────────────────────────────────────────┘   │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│         MICROLOOP ENGINE (dependency)         │
│  HistoryTracker — 460ns exact detection       │
│  RuleEngine — Regex/Exact/JsonSchema matching │
│  Canonical — Volatile field stripping         │
│  Auto-inference — High-entropy field detection│
└───────────────────────────────────────────────┘
```

## Data Flow

1. An agent makes a **tool call** through a framework (LangGraph, CrewAI, etc.)
2. The **framework adapter** intercepts the call and creates a `ToolCallEvent`
3. The **Orchestrator** receives the event:
   a. Stores it in the bounded **history store** (default: 100 entries)
   b. Runs the **observation pipeline** (all detectors)
   c. If detections fire, evaluates **policies** against them
   d. Returns an **Intervention** (or None if no loop detected)
4. The **adapter** executes the intervention (or downgrades unsupported actions)

## Separation Boundary

| Layer | Responsibility | Location |
|-------|----------------|----------|
| **Microloop** | Detection engine (HistoryTracker, RuleEngine, Canonical) | `../microloop` (crate dependency) |
| **deadband-observation** | Detector trait, pipeline, events, auto-inference | `crates/deadband-observation/` |
| **deadband-core** | Orchestrator, policy, intervention, replay, metrics | `crates/deadband-core/` |
| **Adapters** | Framework-specific wrappers | `crates/deadband-adapter-*/` |
| **CLI** | Command-line interface | `cli/` |
| **Python** | PyO3 bindings | `python/deadband/` |
