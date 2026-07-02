"""
Loopless — execution runtime for AI agents.

Detects agent loops, intervenes intelligently, and recovers execution.

Usage:
    from loopless import Orchestrator

    orch = Orchestrator("loopless.yaml")
    intervention = orch.process("session-1", 0, "get_revenue", '{"year": 2024}')
    if intervention and intervention.is_abort():
        print(f"Aborted: {intervention.reason()}")
"""

from ._rust import Orchestrator as _Orchestrator
from ._rust import Intervention as _Intervention
from ._rust import RecoveryMetrics as _RecoveryMetrics
from typing import Optional


class Orchestrator:
    """Loopless orchestrator — detects loops and intervenes in agent tool calls."""

    def __init__(self, config_path: str):
        with open(config_path) as f:
            self._inner = _Orchestrator(f.read())

    def process(
        self,
        thread_id: str,
        step: int,
        tool_name: str,
        arguments: str,
    ) -> Optional[_Intervention]:
        """Process a tool call event and return an intervention if needed.

        Args:
            thread_id: Session/thread identifier.
            step: Monotonic step counter.
            tool_name: Name of the tool being called.
            arguments: JSON string of tool arguments.

        Returns:
            Intervention if a loop was detected, None otherwise.
        """
        return self._inner.process(thread_id, step, tool_name, arguments)

    @property
    def metrics(self) -> _RecoveryMetrics:
        """Recovery metrics for this orchestrator session."""
        return self._inner.get_metrics()


__all__ = ["Orchestrator"]
