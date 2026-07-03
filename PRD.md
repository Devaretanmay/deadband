# Deadband — Product Requirements Document (PRD)

**Version:** 1.0
**Date:** July 2, 2026
**Status:** Final — Ready for Implementation

---

## 1. Executive Summary

### 1.1 Product Vision

**Deadband is an execution runtime that sits between AI agents and their tools, continuously deciding whether execution should continue, adapt, or stop.**

Current AI agents are stateless. They don't remember how they fail. They can't update their own behavior. They loop, drift, and burn money — often undetected until the bill arrives. A single 24-hour agent loop can cost **$4,000+**. One documented case cost a team **$47,000** from an 11-day undetected loop.

Deadband solves this by providing a lightweight, local-first, sub-millisecond execution runtime that observes every tool call, detects failures, and intervenes before execution spirals out of control.

### 1.2 Product Mission

> "Make AI agents reliable by making their execution observable, controllable, and recoverable."

### 1.3 The Problem

| Issue | Impact |
|-------|--------|
| Agents have no memory of failure | Same errors repeat indefinitely |
| No runtime introspection | Problems invisible until cost spikes |
| Loop detection is reactive | Money already spent by the time you notice |
| Frameworks lack recovery mechanisms | Hard termination kills productive reasoning |
| Semantic loops go undetected | Different tools, same intent — no one catches it |

### 1.4 The Solution

**Deadband = Event-driven execution runtime with intervention capabilities**

| Component | What It Does |
|-----------|--------------|
| **Microloop** | Detects what happened (events, detection, semantic analysis) |
| **Deadband** | Decides what to do about it (policies, interventions, orchestration) |
| **Adapters** | Integrates with LangGraph, CrewAI, OpenAI Agents SDK |
| **CLI** | Doctor, trace, replay, inspect |
| **Replay** | Full execution replay for debugging |

### 1.5 Key Differentiators

- **Execution runtime, not a dashboard** — Active intervention, not passive monitoring
- **Local-first** — 100% on-premise, no data leakage, no cloud costs
- **Sub-millisecond overhead** — Built in Rust, not Python
- **Semantic detection** — Catches intent loops, not just exact repeats
- **Replay debugger** — Full execution traces for debugging
- **Extensible** — Detector and Policy plugin APIs

---

## 2. Product Goals

### 2.1 Primary Goals

1. **Detect agent loops** — Exact, semantic, rule-based, and error-pattern detection
2. **Intervene intelligently** — Retry, backoff, replace tool, inject prompt, or abort
3. **Recover execution** — Get the agent back on a working path
4. **Provide visibility** — CLI tools for trace, replay, inspect
5. **Framework-agnostic** — LangGraph, CrewAI, OpenAI Agents SDK

### 2.2 Success Metrics

| Metric | Target (v0.1) | Target (v1.0) |
|--------|---------------|---------------|
| Detection accuracy | >95% | >99% |
| Recovery success rate | >80% | >95% |
| Overhead per tool call | <5ms | <2ms |
| Framework integrations | 3 | 6+ |
| Developer adoption | 10 users | 500+ users |
| Intervention precision | <5% false positives | <2% false positives |

### 2.3 Non-Goals (v0.1)

- Cloud/telemetry backend
- Enterprise RBAC
- Multi-tenant dashboard
- Billing or payment system
- ML model training
- AI-generated prompts
- Checkpoint restore (future)

---

## 3. Product Architecture

### 3.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        AGENT                               │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                   LOOPLESS RUNTIME                         │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              FRAMEWORK ADAPTER                      │   │
│  │  LangGraph │ CrewAI │ OpenAI Agents                │   │
│  │  Converts framework events → ToolCallEvent         │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              ORCHESTRATOR                          │   │
│  │  1. Receive ToolCallEvent                         │   │
│  │  2. Call Microloop.detect()                      │   │
│  │  3. Get Detection                                │   │
│  │  4. Evaluate Policies                            │   │
│  │  5. Produce Intervention                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              POLICY ENGINE                         │   │
│  │  YAML/TOML configuration                           │   │
│  │  Condition evaluators                              │   │
│  │  Action converters                                 │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     MICROLOOP ENGINE                        │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              EVENT ENGINE                          │   │
│  │  ToolCallEvent (versioned)                         │   │
│  │  Error classification                              │   │
│  │  History tracking                                  │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              DETECTORS                             │   │
│  │  ExactDetector → ExactRepeat                       │   │
│  │  SemanticDetector → SemanticRepeat                 │   │
│  │  RuleDetector → RuleViolation                      │   │
│  │  HistoryDetector → ErrorPattern                    │   │
│  │  BudgetDetector → BudgetExceeded                   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                         TOOLS                              │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Component Responsibilities

| Component | Responsibility |
|-----------|---------------|
| **Event Engine** | Defines ToolCallEvent, ErrorKind |
| **Detectors** | Detect what happened |
| **Semantic Sidecar** | 100MB embedding for intent detection |
| **Orchestrator** | Event -> Detection -> Policy -> Intervention |
| **Policy Engine** | Evaluate YAML policies |
| **Framework Adapters** | Framework-specific integrations |
| **Replay** | Serialize/deserialize execution traces |
| **CLI** | Doctor, trace, replay, inspect |

### 3.3 Separation Boundary

```
MICROLOOP = "What happened?"
  - Event model
  - Detection enum
  - Detector plugins
  - Semantic analysis

LOOPLESS = "What should we do?"
  - Intervention protocol
  - Policy engine
  - Orchestration
  - Framework adapters
  - Replay
  - CLI
```

**Microloop never depends on Deadband.** Deadband depends on Microloop.

---

## 4. Feature Requirements

### 4.1 Feature: Event System

**Description:** Define a versioned event model that captures every tool call, result, and failure.

**User Stories:**
- As a developer, I want every tool call to generate an event so I can trace execution.
- As a developer, I want events to be versioned so future changes don't break my traces.
- As a developer, I want events to include timestamps and IDs so I can correlate them.

**Acceptance Criteria:**
- [ ] `ToolCallEvent` struct with version, id, timestamp, thread_id, step
- [ ] Payload enum: Started, Succeeded, Failed
- [ ] ErrorKind enum: Timeout, Validation, Permission, NotFound, Network, RateLimit, Internal, Semantic, Unknown
- [ ] UUID generation for each event
- [ ] Serialization/deserialization (serde)
- [ ] Unit tests for all variants
- [ ] Version field (starting at 1)

**Priority:** P0 (Critical)

### 4.2 Feature: Detection Engine

**Description:** Detect what happened during execution — exact repeats, semantic loops, rule violations, error patterns.

**User Stories:**
- As a developer, I want to detect exact tool call repeats so I can catch obvious loops.
- As a developer, I want to detect semantic repeats so I can catch intent loops.
- As a developer, I want to define custom rules so I can enforce tool usage policies.
- As a developer, I want to extend detection via plugins so I can add custom detectors.

**Acceptance Criteria:**
- [ ] `Detection` enum: ExactRepeat, SemanticRepeat, RuleViolation, ErrorPattern, BudgetExceeded
- [ ] `Detector` trait with detect() method
- [ ] ExactDetector with configurable threshold
- [ ] SemanticDetector that calls semantic sidecar
- [ ] RuleDetector with regex, exact, schema rules
- [ ] HistoryDetector for error patterns
- [ ] `DetectorPlugin` trait for custom detectors
- [ ] Unit tests for each detector

**Feature: Auto-Inference of Volatile Fields**

**Description:** Automatically detect high-entropy fields (req_id, timestamp, session_token) that change on every call and exclude them from loop matching.

**Implementation:** Ported from `microloop::canonical`. The `ExactDetector` runs `auto_infer_volatile_fields()` before matching, comparing arguments against history to identify fields that differ in every prior call while the rest remain identical. Fields must differ in at least 2 prior calls to avoid false positives.

**Acceptance Criteria:**
- [ ] Fields like req_id, timestamp auto-excluded from exact matching
- [ ] Configuration flag to disable (`with_auto_inference(false)`)
- [ ] Exposed via Python as `canonicalize_args(args_json, volatile_fields)`
- [ ] Unit tests for auto-inference logic

**Priority:** P0 (Critical)

**Feature: Shadow Mode for Semantic Sidecar**

**Description:** When the semantic sidecar is unreachable, Deadband gracefully degrades: logs a warning, falls back to exact detection only, and tracks shadow metrics (unavailability count, shadow mode active) so users can measure the impact of running without the sidecar.

**Implementation:** `SemanticSidecarClient` wraps `SidecarShadowMetrics` in a `Mutex` for interior mutability. On sidecar error: `tracing::warn!` logged, `sidecar_unavailable_count` incremented, `Detect()` returns `None` (exact-only fallback). On recovery: `tracing::info!` logged, metrics reset. Exposed via `SemanticDetector.shadow_metrics()` and `is_shadow_mode()`.

**Acceptance Criteria:**
- [ ] Warning logged when sidecar goes down
- [ ] Info logged when sidecar recovers
- [ ] Exact detection continues during shadow mode
- [ ] Shadow metrics queryable at runtime
- [ ] No data loss during sidecar outage

**Priority:** P1 (High)

### 4.3 Feature: Semantic Sidecar

**Description:** Optional 100MB embedding model for intent-based semantic loop detection.

**User Stories:**
- As a developer, I want to catch semantic loops so I can prevent intent-based repeats.
- As a developer, I want the sidecar to be optional so I can use it on resource-constrained systems.

**Acceptance Criteria:**
- [ ] HTTP server on localhost:8081
- [ ] `/analyze` endpoint accepting tool + args
- [ ] Returns cosine similarity with last 5 embeddings
- [ ] Uses sentence-transformers/all-MiniLM-L6-v2
- [ ] Configurable similarity threshold (default 0.85)
- [ ] 100% local, no cloud dependencies
- [ ] Graceful fallback if sidecar is not running

**Priority:** P1 (High)

### 4.4 Feature: Intervention Protocol

**Description:** Define extensible interventions for what to do about a detection.

**User Stories:**
- As a developer, I want to retry a tool call with optional delay so I can handle transient failures.
- As a developer, I want to inject guidance prompts so I can redirect the agent.
- As a developer, I want to replace tools so I can switch to a working alternative.
- As a developer, I want to extend interventions so I can add custom recovery actions.

**Acceptance Criteria:**
- [ ] `Intervention` enum: Continue, Retry, Backoff, ReplaceTool, InjectPrompt, Abort
- [ ] `PromptPosition` enum: BeforeNext, ReplaceLast, AfterTool
- [ ] Extensible via Custom variant
- [ ] Serialization/deserialization
- [ ] Helper methods: is_continue(), is_abort()
- [ ] Unit tests for all variants

**Priority:** P0 (Critical)

### 4.5 Feature: Orchestrator

**Description:** Orchestrate the pipeline: Event -> Detection -> Policy -> Intervention.

**User Stories:**
- As a developer, I want to receive an Intervention for any event so I can decide what to do.
- As a developer, I want to see the full event history so I can debug execution.
- As a developer, I want to see metrics for each intervention so I can measure impact.

**Acceptance Criteria:**
- [ ] `Orchestrator` struct with process(event) -> Option<Intervention>
- [ ] Event history with bounded size (configurable, default 100)
- [ ] Detection pipeline: exact -> semantic -> rules -> history
- [ ] Policy evaluation loop
- [ ] Metrics collection: intervention_count, prevented_calls, recovery_time
- [ ] Unit tests for pipeline

**Priority:** P0 (Critical)

### 4.6 Feature: Policy Engine (YAML)

**Description:** Define policies in YAML for behavior configuration without recompilation.

**User Stories:**
- As a developer, I want to configure policies in YAML so I can change behavior without recompiling.
- As a developer, I want to define conditions based on repeat count, error type, semantic drift, or rules.
- As a developer, I want to combine conditions with AND/OR so I can build complex policies.

**Acceptance Criteria:**
- [ ] YAML parser for policies
- [ ] Condition enum: RepeatCount, ErrorType, SemanticDrift, RuleViolation, Any, All
- [ ] Action enum: Retry, Backoff, ReplaceTool, InjectPrompt, Abort
- [ ] Policy evaluation with matching logic
- [ ] Validation for infinite loops
- [ ] Unit tests for all conditions

**Priority:** P1 (High)

### 4.7 Feature: Framework Adapters

**Description:** Integrate Deadband with LangGraph, CrewAI, and OpenAI Agents SDK.

**User Stories:**
- As a LangGraph user, I want to add Deadband as middleware so I don't need to change my existing code.
- As a CrewAI user, I want to inherit from DeadbandCrewAIFlow so I get automatic protection.
- As an OpenAI Agents user, I want to wrap my tools so I get interception.

**Acceptance Criteria:**
- [ ] `DeadbandLangGraphMiddleware` with wrap_tool_node()
- [ ] `DeadbandCrewAIFlow` with method wrapping
- [ ] `DeadbandOpenAIAgentsMiddleware` with tool wrapping
- [ ] Thread-safe orchestrator instance
- [ ] Examples for each framework

**Priority:** P1 (High)

### 4.8 Feature: Replay

**Description:** Serialize and replay execution traces for debugging.

**User Stories:**
- As a developer, I want to save execution traces so I can debug later.
- As a developer, I want to replay traces so I can reproduce issues.
- As a developer, I want to compare original and replayed traces so I can find discrepancies.

**Acceptance Criteria:**
- [ ] `Trace` struct with execution_id, events, interventions, metrics
- [ ] Serialization to JSON/MessagePack
- [ ] `Replayer` with replay() and compare() methods
- [ ] Metrics: total_events, interventions, prevented_calls, recovery_time
- [ ] Integration with CLI

**Priority:** P1 (High)

### 4.9 Feature: CLI

**Description:** Command-line interface for Deadband operations.

**User Stories:**
- As a developer, I want to check if Deadband is working so I know my setup is correct.
- As a developer, I want to trace an execution so I can see what's happening.
- As a developer, I want to replay a trace so I can debug issues.
- As a developer, I want to inspect a trace in detail so I can understand the timeline.

**Acceptance Criteria:**
- [ ] `deadband doctor` — health check
- [ ] `deadband trace` — start tracing
- [ ] `deadband replay trace.json` — replay a trace
- [ ] `deadband inspect trace.json` — detailed view
- [ ] `deadband visualize trace.json` — ASCII timeline
- [ ] Colored output
- [ ] CLI help and documentation

**Priority:** P2 (Medium)

### 4.10 Feature: Metrics

**Description:** Collect and expose recovery metrics.

**User Stories:**
- As a developer, I want to see how many interventions occurred so I can measure impact.
- As a developer, I want to see how many tool calls were prevented so I can measure cost savings.
- As a developer, I want to see recovery time so I can measure performance.

**Acceptance Criteria:**
- [ ] `RecoveryMetrics` struct
- [ ] Fields: execution_id, runtime_ms, intervention_count, recovery_time_ms, prevented_calls, loop_duration_ms
- [ ] Auto-collected during orchestration
- [ ] Exposed via CLI inspect
- [ ] Export to JSON

**Priority:** P2 (Medium)

---

## 5. User Stories

### 5.1 Developer — Core Usage

| ID | Story | Priority |
|----|-------|----------|
| US-1 | As a developer, I want to install Deadband with `pip install deadband` so I can get started quickly. | P0 |
| US-2 | As a developer, I want to configure Deadband with a YAML file so I can define policies without code. | P1 |
| US-3 | As a developer, I want Deadband to intervene when a loop is detected so I don't burn API credits. | P0 |
| US-4 | As a developer, I want to see what Deadband did so I can verify it worked. | P1 |
| US-5 | As a developer, I want to replay execution traces so I can debug issues. | P1 |

### 5.2 Developer — Framework Integration

| ID | Story | Priority |
|----|-------|----------|
| US-6 | As a LangGraph user, I want to add Deadband middleware so I don't change my existing code. | P1 |
| US-7 | As a CrewAI user, I want to inherit DeadbandCrewAIFlow so I get automatic protection. | P1 |
| US-8 | As an OpenAI Agents user, I want to wrap my tools so I get interception. | P1 |

### 5.3 Developer — CLI

| ID | Story | Priority |
|----|-------|----------|
| US-9 | As a developer, I want to run `deadband doctor` so I can check my setup. | P2 |
| US-10 | As a developer, I want to run `deadband trace` so I can see execution flow. | P2 |
| US-11 | As a developer, I want to run `deadband replay trace.json` so I can debug. | P2 |
| US-12 | As a developer, I want to run `deadband inspect trace.json` so I can see details. | P2 |

### 5.4 Developer — Customization

| ID | Story | Priority |
|----|-------|----------|
| US-13 | As a developer, I want to write custom detectors so I can extend Deadband. | P2 |
| US-14 | As a developer, I want to write custom policies so I can define custom recovery behavior. | P2 |
| US-15 | As a developer, I want to create custom framework adapters so I can integrate with my stack. | P3 |

---

## 6. Acceptance Criteria

### 6.1 General

- [ ] All Rust code passes `cargo test`
- [ ] All Python code passes `pytest`
- [ ] CI pipeline passes
- [ ] Documentation is complete
- [ ] Examples are working

### 6.2 Performance

- [ ] Detection overhead < 5ms per tool call
- [ ] Semantic detection < 10ms (including sidecar)
- [ ] Memory overhead < 150MB (with semantic sidecar)
- [ ] No external API calls

### 6.3 Security

- [ ] No data leaves the local environment
- [ ] No network calls except optional sidecar
- [ ] Sensitive data (tools, arguments) not logged unless debug enabled

### 6.4 Compatibility

- [ ] Python 3.9+
- [ ] Rust 1.75+
- [ ] LangGraph 0.2+
- [ ] CrewAI 0.70+
- [ ] OpenAI Agents SDK 0.1+

---

## 7. Dependencies

### 7.1 External Dependencies (Rust)

| Dependency | Purpose |
|------------|---------|
| serde | Serialization |
| serde_json | JSON support |
| uuid | Event IDs |
| clap | CLI parsing |
| thiserror | Error handling |
| tokio | Async runtime |
| pyo3 | Python bindings |
| serde_yaml | YAML policies |
| chrono | Timestamps |
| reqwest | HTTP client (semantic sidecar) |

### 7.2 External Dependencies (Python)

| Dependency | Purpose |
|------------|---------|
| langgraph | Framework integration |
| crewai | Framework integration |
| openai-agents | Framework integration |

### 7.3 Internal Dependencies

| Component | Dependency |
|-----------|------------|
| Deadband | Microloop (crate) |
| Deadband Python | Deadband Rust (PyO3) |
| Framework Adapters | Deadband Python |

---

## 8. Timeline and Milestones

### 8.1 Phase 0: Foundation (Week 1)

| Epic | Deliverable | Days |
|------|-------------|------|
| Epic 1 | Event Model | 2 |
| Epic 2 | Detection Engine | 2 |
| Epic 3 | Error Classification | 1 |
| **Milestone** | Microloop core extension complete | **5 days** |

### 8.2 Phase 1: Core Runtime (Week 2-3)

| Epic | Deliverable | Days |
|------|-------------|------|
| Epic 4 | Intervention Protocol | 2 |
| Epic 5 | Orchestrator | 3 |
| Epic 6 | Policy Engine | 2 |
| **Milestone** | Deadband core complete | **7 days** |

### 8.3 Phase 2: Framework Integration (Week 4-5)

| Epic | Deliverable | Days |
|------|-------------|------|
| Epic 7 | LangGraph Adapter | 3 |
| Epic 8 | OpenAI Agents SDK Adapter | 2 |
| Epic 9 | CrewAI Adapter | 2 |
| **Milestone** | 3 framework integrations complete | **7 days** |

### 8.4 Phase 3: Developer Experience (Week 6-7)

| Epic | Deliverable | Days |
|------|-------------|------|
| Epic 10 | CLI | 3 |
| Epic 11 | Replay | 2 |
| Epic 12 | Metrics | 1 |
| **Milestone** | CLI + Replay complete | **6 days** |

### 8.5 Phase 4: Launch (Week 8)

| Epic | Deliverable | Days |
|------|-------------|------|
| Epic 13 | Demo Page | 1 |
| Epic 14 | Documentation | 2 |
| Epic 15 | v0.1 Release | 1 |
| **Milestone** | v0.1 released | **4 days** |

**Total Timeline: 8 weeks (~30 days)**

---

## 9. Success Criteria

### 9.1 Technical Success

- [ ] 3 framework integrations working (LangGraph, CrewAI, OpenAI Agents)
- [ ] Detection accuracy > 95%
- [ ] Recovery success > 80%
- [ ] Overhead < 5ms per call
- [ ] All tests passing
- [ ] Documentation complete

### 9.2 Adoption Success

- [ ] 10+ developers using Deadband
- [ ] 100+ GitHub stars
- [ ] 5+ issues opened (indicates usage)
- [ ] 1+ community contribution

### 9.3 Business Success

- [ ] Clear value proposition validated
- [ ] Demonstrated recovery in real workflows
- [ ] Identified 3+ enterprise prospects
- [ ] $0 spent on infrastructure

---

## 10. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Semantic sidecar performance | Medium | High | Keep optional; use efficient embeddings |
| Framework changes breaking adapters | Medium | Medium | Write minimal adapters; version tightly |
| Recovery prompts ineffective | Medium | High | Iterate based on user feedback; make configurable |
| Adoption slow | Medium | Medium | Show demo prominently; target one framework first |
| Competition emerges | Medium | Medium | Moat is adoption; ship fast |

---

## 11. Open Questions

1. **Q:** Should we support other frameworks (e.g., AutoGen) in v0.1?
   **A:** No. Start with LangGraph, CrewAI, OpenAI Agents. Add others based on demand.

2. **Q:** Should we expose a REST API?
   **A:** Not in v0.1. Focus on native integrations.

3. **Q:** Should we support event streaming?
   **A:** Not in v0.1. Collect events in memory; export via CLI.

4. **Q:** Should we support checkpoint/restore?
   **A:** Not in v0.1. Focus on intervention; add later.

---

## 12. Glossary

| Term | Definition |
|------|------------|
| **Agent** | Autonomous program that uses LLMs to take actions |
| **Tool** | Function or API that an agent can call |
| **Loop** | Repeated execution of the same or semantically similar tool calls |
| **Detection** | Recognition that a loop or error pattern has occurred |
| **Intervention** | Action taken to recover execution |
| **Adapter** | Framework-specific integration layer |
| **Trace** | Serialized execution history |
| **Replay** | Re-execution of a trace |
| **Orchestrator** | Core component that processes events and produces interventions |
| **Policy** | User-defined rule that maps detections to interventions |
| **Semantic Sidecar** | 100MB embedding model for intent detection |

---

## 13. Appendix

### A. Example Use Cases

**Use Case 1: Database Timeout Loop**

1. Agent calls `get_revenue()` -> timeout
2. Agent calls `get_revenue()` -> timeout
3. Agent calls `get_revenue()` -> timeout
4. **Deadband detects 3 repeats**
5. **Deadband injects prompt:** "Database is down. Use cached summary."
6. Agent calls `get_cached_revenue()` -> success
7. **Recovered in 84ms, 14 tool calls prevented**

**Use Case 2: Semantic Loop**

1. Agent calls `delete_line(5)` -> fails (permission)
2. Agent calls `remove_line(5)` -> fails (permission)
3. Agent calls `erase_line(5)` -> fails (permission)
4. **Deadband detects semantic drift > 0.85**
5. **Deadband injects prompt:** "You don't have permission to delete. Try commenting out instead."
6. Agent calls `comment_line(5)` -> success
7. **Recovered, tool call avoided**

### B. Example Configuration

```yaml
# deadband.yaml
policies:
  - name: "hard_fork_on_repeats"
    when:
      type: "RepeatCount"
      params:
        threshold: 3
    do:
      type: "InjectPrompt"
      params:
        content: "You've tried this tool 3 times. Use the backup approach."
        position: "ReplaceLast"

  - name: "retry_timeout"
    when:
      type: "ErrorType"
      params:
        kind: "Timeout"
        threshold: 1
    do:
      type: "Retry"
      params:
        delay_ms: 100

  - name: "semantic_nudge"
    when:
      type: "SemanticDrift"
      params:
        threshold: 0.85
    do:
      type: "InjectPrompt"
      params:
        content: "You're trying similar approaches. Consider a different strategy."
        position: "BeforeNext"
```

---

## 14. Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Product Owner | [Name] | [Date] | [Signature] |
| Engineering Lead | [Name] | [Date] | [Signature] |
| Stakeholder | [Name] | [Date] | [Signature] |

---

**Document Status:** Ready for Implementation

**Next Steps:**
1. Create GitHub issues for each epic
2. Begin Phase 0 (Epic 1: Event System)
3. Ship weekly updates
4. Track success metrics
