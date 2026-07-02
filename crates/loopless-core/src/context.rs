use loopless_observation::event::ToolCallEvent;
use loopless_observation::report::DetectionReport;

use crate::intervention::AdapterCapabilities;
use crate::metrics::RecoveryMetrics;
use crate::orchestrator::OrchestratorConfig;

#[derive(Clone, Debug)]
pub struct ExecutionContext<'a> {
    pub report: &'a DetectionReport,
    pub history: &'a [ToolCallEvent],
    pub metrics: &'a RecoveryMetrics,
    pub config: &'a OrchestratorConfig,
    pub capabilities: &'a AdapterCapabilities,
}
