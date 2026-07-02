from typing import Any, Callable, Dict, List, Optional
from loopless import Orchestrator


class LooplessOpenAIToolWrapper:
    def __init__(self, orchestrator: Orchestrator):
        self._orchestrator = orchestrator
        self._step = 0

    def wrap_tools(self, tools: List[Callable]) -> List[Callable]:
        wrapped = []
        for tool in tools:
            wrapped.append(self._wrap_tool(tool))
        return wrapped

    def _wrap_tool(self, tool: Callable) -> Callable:
        def wrapped(**kwargs: Any) -> Any:
            name = getattr(tool, "__name__", str(tool))
            intervention = self._orchestrator.process(
                name, kwargs,
                thread_id="openai-agent",
                step=self._step,
            )
            self._step += 1
            if intervention is not None:
                if intervention.get("type") == "abort":
                    raise RuntimeError(f"Loopless aborted: {intervention.get('reason', 'unknown')}")
            return tool(**kwargs)
        return wrapped
