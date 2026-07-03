pub mod context;
pub mod fingerprint;
pub mod gaslighter;
pub mod intervention;
pub mod metrics;
pub mod orchestrator;
pub mod policy;
pub mod replay;

pub use deadband_observation::detection::{
    BudgetDetector, CompiledRule, Detection, Detector, DetectorBox, ExactDetector,
    HistoryDetector, RuleDetector, SemanticDetector, SemanticSidecarClient,
};
pub use deadband_observation::event::{ErrorKind, Payload, ToolCallEvent};
pub use deadband_observation::pipeline::{ObservationPipeline, PipelineConfig};
pub use deadband_observation::report::{DetectionReport, ObservationMetadata};
pub use deadband_observation::history::{HistoryStore, InMemoryHistoryStore};

/// Canonicalize tool arguments by stripping volatile fields.
/// Returns the cleaned JSON string.
pub fn canonicalize_args(args_json: &str, volatile_fields: &[String]) -> String {
    deadband_observation::canonicalize_args(args_json, volatile_fields)
}

pub use context::ExecutionContext;
pub use intervention::{AdapterCapabilities, Intervention, InterventionOutcome, PromptPosition};
pub use metrics::{MetricEvent, MetricsSnapshot, RecoveryMetrics, VitalSigns};
pub use fingerprint::FingerprintStore;
pub use gaslighter::{Gaslighter, GaslighterTrial, PromptStats};
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use policy::PolicyEngine;
pub use replay::Replayer;
