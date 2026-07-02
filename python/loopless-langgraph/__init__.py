from typing import Any, Callable, Dict, List, Optional
from loopless import Orchestrator


class LooplessLangGraphMiddleware:
    def __init__(self, orchestrator: Orchestrator):
        self._orchestrator = orchestrator
        self._step = 0

    def wrap_tool_node(self, tool_node: Callable) -> Callable:
        def wrapped(state: Dict[str, Any]) -> Dict[str, Any]:
            result = tool_node(state)
            messages = state.get("messages", [])
            for msg in messages:
                if hasattr(msg, "tool_calls") and msg.tool_calls:
                    for tc in msg.tool_calls:
                        intervention = self._orchestrator.process(
                            tc["name"], tc.get("args", {}),
                            thread_id=state.get("thread_id", "default"),
                            step=self._step,
                        )
                        self._step += 1
                        if intervention is not None:
                            if intervention.get("type") == "abort":
                                raise RuntimeError(
                                    f"Loopless aborted: {intervention.get('reason', 'unknown')}"
                                )
                            if intervention.get("type") == "inject_prompt":
                                msg.content = (
                                    f"{intervention['content']}\n\n{msg.content or ''}"
                                )
            return result

        return wrapped
