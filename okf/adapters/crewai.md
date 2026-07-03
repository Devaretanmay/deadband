---
type: Integration
title: CrewAI Adapter
description: Flow subclass that automatically protects CrewAI agents from loops.
tags: [deadband, crewai, adapter, integration]
timestamp: 2026-07-03T00:00:00Z
---

# CrewAI Adapter

The CrewAI adapter provides a `LooplessCrewAIFlow` subclass that
automatically intercepts tool calls for loop protection.

## Usage

```rust
use deadband_adapter_crewai::LooplessCrewAIFlow;
use deadband_core::Orchestrator;

let orch = Orchestrator::from_yaml(yaml).unwrap();
let mut flow = LooplessCrewAIFlow::new(orch);

if let Some(intervention) = flow.intercept_tool_call(
    "search",
    r#"{"q": "hello"}"#,
    "session-1",
) {
    // Handle intervention
}
```

## AdapterCapabilities

| Capability | Supported |
|------------|-----------|
| Retry |  |
| Abort |  (resets step counter) |
| InjectPrompt |  |
| ReplaceTool |  |
