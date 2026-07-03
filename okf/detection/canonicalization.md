---
type: Feature
title: Argument Canonicalization
description: Normalizes tool arguments for consistent matching by stripping volatile fields.
tags: [deadband, detection, canonical, normalization]
timestamp: 2026-07-03T00:00:00Z
---

# Argument Canonicalization

**Canonicalization** normalizes tool arguments before comparison to
ensure that semantically identical calls are recognized as repeats even
when volatile metadata fields differ.

## Usage

### Rust

```rust
use deadband_observation::canonicalize_args;

let cleaned = canonicalize_args(
    r#"{"query": "hello", "req_id": 123}"#,
    &["req_id".to_string()],
);
```

### Python

```python
from deadband import canonicalize_args

cleaned = canonicalize_args(
    '{"query": "hello", "req_id": 123}',
    ["req_id"]
)
# Returns: '{"query":"hello"}'
```

## How It Works

The function parses the JSON arguments, strips fields matching the
provided paths using dot-notation (supporting nested paths like
`meta.timestamp`), and returns the cleaned JSON string.

## Integration with Auto-Inference

When auto-inference is enabled, the `ExactDetector` automatically
determines which fields to canonicalize — you don't need to specify
them manually. The `canonicalize_args` function is useful for:

- **Debugging** — See exactly what fields get stripped
- **Previewing** — Verify auto-inference behavior
- **Pre-processing** — Clean arguments before sending to other systems
