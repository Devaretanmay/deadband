from typing import Any, Dict, Optional
from loopless import Orchestrator


class LooplessCrewAIFlow:
    def __init__(self, orchestrator: Orchestrator, *args, **kwargs):
        self._orchestrator = orchestrator
        self._step = 0

    def intercept_tool_call(self, tool_name: str, arguments: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        intervention = self._orchestrator.process(
            tool_name, arguments,
            thread_id=self.__class__.__name__,
            step=self._step,
        )
        self._step += 1
        return intervention
