# Deadband

**Execution runtime for AI agents — detect loops, intervene intelligently, recover execution.**

Deadband sits between AI agents and their tools, observing every tool call and deciding whether execution should continue, adapt, or stop. It combines the **Microloop** battle-tested detection engine (460ns per check) with a rich intervention layer (retry, backoff, replace tool, inject prompt, abort).

## Quick Start

```bash
pip install deadband
```

```python
from deadband import Orchestrator, canonicalize_args

# Initialize with a YAML policy file
orchestrator = Orchestrator("deadband.yaml")

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
Deadband imports [Microloop](https://github.com/Devaretanmay/microloop) as its detection backend. The `ExactDetector` delegates to `microloop::HistoryTracker` for **460ns** loop detection, and `RuleDetector` delegates Regex/Exact/JsonSchema matching to `microloop::engine::CompiledRule`.

### 🧠 Auto-Inference of Volatile Fields
Fields like `req_id`, `timestamp`, or `session_token` that change on every call can fool naive detectors. Deadband's `ExactDetector` automatically detects these high-entropy fields and excludes them from comparison — catching loops that would otherwise slip through.

```python
# Auto-inference works automatically. Disable with:
# ExactDetector::with_auto_inference(false)
cleaned = canonicalize_args('{"query": "python", "req_id": 42}', ["req_id"])
# Returns: {"query":"python"}
```

### 📡 Semantic Detection (Optional Sidecar)
An optional sidecar server (`microloop-semantic`) provides intent-based loop detection using BERT embeddings. When the sidecar is unreachable, Deadband enters **shadow mode** — logging warnings, falling back to exact-only detection, and tracking metrics so you know what you're missing.

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
When error patterns are detected, Deadband automatically tightens the repeat threshold (e.g., 3 → 2), forcing agents to pivot faster when they're stuck in a failing pattern. This mirrors Microloop's proxy behavior.

### 📊 Trace & Replay
Every execution can be saved as a versioned JSON trace for later debugging:

```bash
deadband trace < trace.jsonl
deadband replay trace.json
deadband inspect trace.json
deadband visualize trace.json  # ASCII timeline
```

## Framework Integrations

| Framework | Integration |
|-----------|-------------|
| **LangGraph** | `DeadbandLangGraphMiddleware` wraps tool nodes |
| **CrewAI** | `DeadbandCrewAIFlow` flow subclass with automatic protection |
| **OpenAI Agents SDK** | `DeadbandOpenAIToolWrapper` for tool interception |

## Deadband Proxy

Deadband includes an HTTP proxy that intercepts API calls from AI coding tools and protects them from loops — no code changes needed.

### Quick Start

```bash
# Build and enable
cargo build -p deadband-proxy
deadband enable

# Auto-discovers and configures Aider, Claude Code, Cursor, and more
# Starts the proxy on port 4399
# Detects loops and injects intervention prompts
```

### Proxy CLI Commands

| Command | Description |
|---------|-------------|
| `deadband enable [--persistent]` | Auto-configure tools and start proxy |
| `deadband disable` | Restore configs and stop proxy |
| `deadband status` | Show proxy status and loop statistics |
| `deadband logs [--tail N] [--follow]` | View proxy logs |
| `deadband monitor` | Live TUI monitoring dashboard |
| `deadband set [--port N] [--config PATH]` | Change proxy settings |
| `deadband proxy [--port N] [--daemon]` | Start proxy server directly |

### Proxy Architecture

```
┌─────────────────────────────────────────────┐
│    AI Tool (Aider, Claude Code, Cursor…)    │
└──────────────────────┬──────────────────────┘
                       │  HTTP → localhost:4399
┌──────────────────────▼──────────────────────┐
│         DEADBAND PROXY                      │
│                                             │
│  Parse request (OpenAI or Anthropic)        │
│  → SSE buffer (first 5 chunks)             │
│  → Run detection (Exact, Semantic, Rules)  │
│  → Apply intervention if loop detected     │
│  → Forward/replay modified stream          │
└──────────────────────┬──────────────────────┘
                       │  HTTP → api.openai.com or api.anthropic.com
┌──────────────────────▼──────────────────────┐
│           LLM API (Upstream)                │
└─────────────────────────────────────────────┘
```

**Supported endpoints:**
- `POST /v1/chat/completions` — OpenAI-compatible (streaming + non-streaming)
- `POST /v1/messages` — Anthropic-compatible (streaming + non-streaming)

**Detection layers active through the proxy:**
- Exact repeat detection (460ns)
- Rule-based detection (argument validation)
- History-based error pattern detection
- Budget-based call count limits
- Semantic detection (via optional sidecar)

### Tool Auto-Discovery

`deadband enable` automatically discovers and configures:

| Tool | Discovery Path |
|------|---------------|
| **Aider** | `.aider.conf.yml` in project or home |
| **Claude Code** | `~/.claude/settings.json` |
| **Cursor** | `~/.config/Cursor/User/settings.json` |
| **Continue** | `~/.continue/config.json` |
| **GitHub Copilot CLI** | Binary detection in PATH |

Configs are backed up to `~/.deadband/backups/` before modification.

### Persistence

```bash
# Install as system service (starts on boot)
deadband enable --persistent
```

Supports:
- **macOS**: launchd (`~/Library/LaunchAgents/com.deadband.proxy.plist`)
- **Linux**: systemd (`/etc/systemd/system/deadband-proxy.service`)
- **Windows**: Planned

## Performance

| Check | Latency |
|-------|---------|
| Exact repeat detection | ~460ns (Microloop-backed) |
| Semantic detection | ~10ms (with sidecar) |
| Full pipeline | < 5ms per event |
| Proxy latency (streaming) | < 50ms added |
| Memory | < 150MB (with semantic sidecar) |

## Python API

```python
from deadband import Orchestrator, canonicalize_args

# Orchestrator — main entry point
orch = Orchestrator("deadband.yaml")
intervention = orch.process("session-1", 0, "search", '{"q": "hello"}')
metrics = orch.metrics
print(metrics.intervention_count, metrics.prevented_calls)

# Canonicalize args — inspect volatile field stripping
cleaned = canonicalize_args('{"query": "x", "ts": 123}', ["ts"])
# → {"query":"x"}
```

## Documentation

See [PRD.md](./PRD.md) for the full product requirements.

## Project Structure

```
deadband/
├── Cargo.toml                     (workspace root)
├── crates/
│   ├── deadband-core/             (orchestrator, policy, intervention)
│   ├── deadband-observation/      (detection, events, pipeline)
│   ├── deadband-proxy/            (HTTP proxy, CLI, tool discovery)
│   ├── deadband-adapter-langgraph/
│   ├── deadband-adapter-crewai/
│   └── deadband-adapter-openai/
├── cli/                           (legacy CLI — use deadband-proxy)
├── python/deadband/               (PyO3 Python bindings)
└── okf/                           (Open Knowledge Format docs)
```
