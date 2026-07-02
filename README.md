# Loopless

**Execution runtime for AI agents — detect loops, intervene intelligently, recover execution.**

Loopless sits between AI agents and their tools, observing every tool call and deciding whether execution should continue, adapt, or stop. It combines the **Microloop** battle-tested detection engine (460ns per check) with a rich intervention layer (retry, backoff, replace tool, inject prompt, abort).

## Quick Start

```bash
pip install loopless
```

```python
from loopless import Orchestrator, canonicalize_args

# Initialize with a YAML policy file
orchestrator = Orchestrator("loopless.yaml")

# Process a tool call — returns an Intervention if loop detected
intervention = orchestrator.process(
    thread_id="session-1",
    step=0,
    tool_name="get_revenue",
    arguments='{"year": 2024}'
)

if intervention and intervention.is_abort():
    print(f"Aborted: {intervention.reason()}")
elif intervention and intervention.is_retry():
    print(f"Retry with {intervention.delay_ms()}ms delay")
elif intervention and intervention.is_inject_prompt():
    print(f"Prompt: {intervention.prompt_content()}")

# Preview what volatile fields get stripped (auto-inference insight)
cleaned = canonicalize_args('{"query": "hello", "req_id": 123}', ["req_id"])
print(f"Cleaned args: {cleaned}")  # {"query":"hello"}
```

## Architecture

```
┌─────────────────────────────────────────┐
│             AGENT                       │
└──────────────────────┬──────────────────┘
                       │
┌──────────────────────▼──────────────────┐
│       FRAMEWORK ADAPTER                 │
│  LangGraph │ CrewAI │ OpenAI Agents     │
└──────────────────────┬──────────────────┘
                       │
┌──────────────────────▼──────────────────┐
│       LOOPLESS ORCHESTRATOR             │
│  Event → Detection → Policy → Action   │
│                                         │
│  ┌──────────────────────────────────┐   │
│  │  DETECTORS (Microloop-powered)   │   │
│  │  ├─ ExactDetector (460ns)        │   │
│  │  ├─ RuleDetector (arg validate)  │   │
│  │  ├─ SemanticDetector (sidecar)   │   │
│  │  ├─ HistoryDetector (patterns)   │   │
│  │  └─ BudgetDetector (call count)  │   │
│  └──────────────────────────────────┘   │
└──────────────────────┬──────────────────┘
                       │
┌──────────────────────▼──────────────────┐
│       INTERVENTION ENGINE               │
│  PolicyEngine → AdapterCapabilities     │
│  Retry │ Backoff │ ReplaceTool          │
│  InjectPrompt │ Abort │ Custom          │
└─────────────────────────────────────────┘
```

## Key Features

### ⚡ Microloop-Powered Detection
Loopless imports [Microloop](https://github.com/Devaretanmay/microloop) as its detection backend. The `ExactDetector` delegates to `microloop::HistoryTracker` for **460ns** loop detection, and `RuleDetector` delegates Regex/Exact/JsonSchema matching to `microloop::engine::CompiledRule`.

### 🧠 Auto-Inference of Volatile Fields
Fields like `req_id`, `timestamp`, or `session_token` that change on every call can fool naive detectors. Loopless's `ExactDetector` automatically detects these high-entropy fields and excludes them from comparison — catching loops that would otherwise slip through.

```python
# Auto-inference works automatically. Disable with:
# ExactDetector::with_auto_inference(false)
cleaned = canonicalize_args('{"query": "python", "req_id": 42}', ["req_id"])
# Returns: {"query":"python"}
```

### 📡 Semantic Detection (Optional Sidecar)
An optional sidecar server (`microloop-semantic`) provides intent-based loop detection using BERT embeddings. When the sidecar is unreachable, Loopless enters **shadow mode** — logging warnings, falling back to exact-only detection, and tracking metrics so you know what you're missing.

### 📋 Policy Engine
Define loop intervention rules in a YAML file — no recompilation needed:

```yaml
policies:
  - name: "hard_fork_on_repeats"
    when:
      count:
        ExactRepeat: 3
    do:
      type: "InjectPrompt"
      params:
        content: "You've tried this tool 3 times. Try a different approach."
        position: "ReplaceLast"

  - name: "retry_timeout"
    when:
      count:
        ErrorPattern: 2
    do:
      type: "Retry"
      params:
        delay_ms: 100

  - name: "abort_on_budget"
    when:
      count:
        ExactRepeat: 10
    do:
      type: "Abort"
      params:
        reason: "Execution exceeded maximum repeat threshold of 10"
```

### 🔄 Adaptive Thresholding
When error patterns are detected, Loopless automatically tightens the repeat threshold (e.g., 3 → 2), forcing agents to pivot faster when they're stuck in a failing pattern. This mirrors Microloop's proxy behavior.

### 📊 Trace & Replay
Every execution can be saved as a versioned JSON trace for later debugging:

```bash
loopless trace < trace.jsonl
loopless replay trace.json
loopless inspect trace.json
loopless visualize trace.json  # ASCII timeline
```

## Framework Integrations

| Framework | Integration |
|-----------|-------------|
| **LangGraph** | `LooplessLangGraphMiddleware` wraps tool nodes |
| **CrewAI** | `LooplessCrewAIFlow` flow subclass with automatic protection |
| **OpenAI Agents SDK** | `LooplessOpenAIToolWrapper` for tool interception |

## CLI

```bash
loopless doctor      # Health check — validates config, checks sidecar
loopless trace       # Start tracing from stdin (JSON lines)
loopless replay      # Replay a saved trace file
loopless inspect     # Detailed view of a trace
loopless visualize   # ASCII timeline visualization
loopless init        # Generate default loopless.yaml
```

## Performance

| Check | Latency |
|-------|---------|
| Exact repeat detection | ~460ns (Microloop-backed) |
| Semantic detection | ~10ms (with sidecar) |
| Full pipeline | < 5ms per event |
| Memory | < 150MB (with semantic sidecar) |

## Python API

```python
from loopless import Orchestrator, canonicalize_args

# Orchestrator — main entry point
orch = Orchestrator("loopless.yaml")
intervention = orch.process("session-1", 0, "search", '{"q": "hello"}')
metrics = orch.metrics
print(metrics.intervention_count, metrics.prevented_calls)

# Canonicalize args — inspect volatile field stripping
cleaned = canonicalize_args('{"query": "x", "ts": 123}', ["ts"])
# → {"query":"x"}
```

## Documentation

See [PRD.md](./PRD.md) for the full product requirements.
