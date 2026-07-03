pub mod context;
pub mod intervention;
pub mod metrics;
pub mod orchestrator;
pub mod policy;
pub mod replay;

pub use deadband_observation::detection::{
    CompiledRule, Detection, Detector, DetectorBox, ExactDetector,
    HistoryDetector, RuleDetector, SemanticDetector, SemanticSidecarClient,
};
pub use deadband_observation::event::{ErrorKind, Payload, ToolCallEvent};
pub use deadband_observation::pipeline::{ObservationPipeline, PipelineConfig};
pub use deadband_observation::report::{DetectionReport, ObservationMetadata};
pub use deadband_observation::history::{HistoryStore, InMemoryHistoryStore};

pub fn canonicalize_args(args_json: &str, volatile_fields: &[String]) -> String {
    deadband_observation::canonicalize_args(args_json, volatile_fields)
}

pub use context::ExecutionContext;
pub use intervention::{AdapterCapabilities, Intervention, InterventionOutcome, PromptPosition};
pub use metrics::{MetricEvent, RecoveryMetrics};
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use policy::PolicyEngine;
pub use replay::Replayer;
