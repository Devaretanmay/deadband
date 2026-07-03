---
type: Feature
title: Auto-Inference of Volatile Fields
description: Automatically detects high-entropy fields (req_id, timestamp) and excludes them from loop matching.
tags: [deadband, detection, auto-inference, canonical]
timestamp: 2026-07-03T00:00:00Z
---

# Auto-Inference of Volatile Fields

Fields like `req_id`, `timestamp`, or `session_token` change on every
tool call — they're **volatile**. Naive exact matching would never catch
loops when these fields differ. Auto-inference solves this.

## How It Works

Before comparing arguments, the `ExactDetector` calls
`auto_infer_volatile_fields()` to analyze the event history:

1. Look at prior calls to the **same tool**
2. Compare arguments field-by-field
3. If **exactly one field** differs consistently across **≥ 2 prior calls**,
   mark it as volatile
4. Strip the inferred volatile field from all comparisons
5. Re-run matching with the cleaned arguments

## Example

```python
# 3 calls with same query but different req_id
call_1: search(query="python", req_id=1)
call_2: search(query="python", req_id=2)
call_3: search(query="python", req_id=3)  # ← WOULD BE CAUGHT

# Without auto-inference: req_id differs → no loop detected (missed!)
# With auto-inference: req_id stripped → query matches → loop caught!
```

## Configuration

Auto-inference is **enabled by default**. Disable with:

```rust
let detector = ExactDetector::new()
    .with_auto_inference(false);
```

## False Positive Prevention

A field must differ in **at least 2 prior calls** to be inferred as
volatile. This prevents single-off comparisons (e.g., legitimate query
changes) from triggering false positives.
