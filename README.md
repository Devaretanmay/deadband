# Loopless

**Execution runtime for AI agents — detect loops, intervene intelligently, recover execution.**

Loopless sits between AI agents and their tools, observing every tool call and deciding whether execution should continue, adapt, or stop.

## Quick Start

```bash
pip install loopless
```

```python
from loopless import Orchestrator

orchestrator = Orchestrator("loopless.yaml")
event = ToolCallEvent("get_revenue", '{"year": 2024}')
intervention = orchestrator.process(event)

if intervention.is_abort():
    print(f"Aborted: {intervention.reason()}")
elif intervention.is_retry():
    print(f"Retry with {intervention.delay_ms()}ms delay")
```

## Framework Integrations

- **LangGraph** — middleware that wraps tool nodes
- **CrewAI** — flow subclass with automatic protection
- **OpenAI Agents SDK** — tool wrapper for interception

## Documentation

See [PRD.md](./PRD.md) for the full product requirements.
