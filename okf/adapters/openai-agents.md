---
type: Integration
title: OpenAI Agents Adapter
description: Tool wrapper that intercepts calls from OpenAI Agents SDK.
tags: [deadband, openai, adapter, integration]
timestamp: 2026-07-03T00:00:00Z
---

# OpenAI Agents Adapter

The OpenAI Agents adapter wraps individual tools with loop detection.

## Usage

```rust
use deadband_adapter_openai::LooplessOpenAIToolWrapper;
use deadband_core::Orchestrator;

let orch = Orchestrator::from_yaml(yaml).unwrap();
let mut wrapper = LooplessOpenAIToolWrapper::new(orch);

// Intercept before each tool call
if let Some(intervention) = wrapper.intercept_tool_call(
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
| Retry | ✅ |
| Abort | ✅ (resets step counter) |
| InjectPrompt | ❌ |
| ReplaceTool | ❌ |
