---
type: Integration
title: LangGraph Adapter
description: Middleware that wraps LangGraph tool nodes for loop detection.
tags: [deadband, langgraph, adapter, integration]
timestamp: 2026-07-03T00:00:00Z
---

# LangGraph Adapter

The LangGraph adapter wraps tool nodes with loop detection middleware.

## Usage

```rust
use deadband_adapter_langgraph::LooplessLangGraphMiddleware;
use deadband_core::Orchestrator;

let orch = Orchestrator::from_yaml(yaml).unwrap();
let mut middleware = LooplessLangGraphMiddleware::new(orch);

if let Some(intervention) = middleware.wrap_tool_call(
    "search",
    r#"{"q": "hello"}"#,
    "session-1",
    0,
) {
    match intervention {
        Intervention::Abort { reason } => {
            // Agent is looping — abort
        }
        Intervention::InjectPrompt { content, .. } => {
            // Redirect agent with guidance
        }
        _ => {}
    }
}
```

## AdapterCapabilities

| Capability | Supported |
|------------|-----------|
| Retry |  |
| Abort |  |
| InjectPrompt | ❌ (downgraded to Abort) |
| ReplaceTool | ❌ (downgraded to Abort) |
