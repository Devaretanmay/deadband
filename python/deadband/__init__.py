from ._rust import Orchestrator as _Orchestrator
from ._rust import Intervention as _Intervention
from ._rust import RecoveryMetrics as _RecoveryMetrics
from ._rust import DetectionReport as _DetectionReport
from typing import Optional, Tuple

class Orchestrator:
    def __init__(self, config_path: str):
        with open(config_path) as f:
            self._inner = _Orchestrator(f.read())

    def process(
        self,
        thread_id: str,
        step: int,
        tool_name: str,
        arguments: str,
    ) -> Tuple[Optional[_Intervention], Optional[_DetectionReport]]:
        return self._inner.process(thread_id, step, tool_name, arguments)

    @property
    def metrics(self) -> _RecoveryMetrics:
        return self._inner.get_metrics()

__all__ = ["Orchestrator"]
