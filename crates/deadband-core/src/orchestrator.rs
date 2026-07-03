
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use deadband_observation::detection::{
    DetectorBox, ExactDetector, HistoryDetector,
    SemanticDetector, SemanticSidecarClient,
};
use deadband_observation::event::ToolCallEvent;
use deadband_observation::history::{HistoryStore, InMemoryHistoryStore};
use deadband_observation::pipeline::{ObservationPipeline, PipelineConfig};
use deadband_observation::report::DetectionReport;

use crate::intervention::{AdapterCapabilities, Intervention};
use crate::metrics::{MetricEvent, RecoveryMetrics};
use crate::policy::PolicyEngine;
use crate::context::ExecutionContext;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    pub history_size: usize,
    pub enable_semantic: bool,
    pub enable_rules: bool,
    pub enable_exact: bool,
    pub error_threshold: u32,
    pub exact_threshold: u32,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            history_size: 100,
            enable_semantic: true,
            enable_rules: true,
            enable_exact: true,
            error_threshold: 1,
            exact_threshold: 1,
        }
    }
}

pub struct Orchestrator {
    config: OrchestratorConfig,
    pipeline: ObservationPipeline,
    policy_engine: PolicyEngine,
    history_store: Box<dyn HistoryStore>,
    metrics: RecoveryMetrics,
}

impl Orchestrator {
    pub fn new(
        config: OrchestratorConfig,
        policy_yaml: &str,
        extra_detectors: Vec<DetectorBox>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let policy_engine = PolicyEngine::new(policy_yaml)?;

        let mut detectors: Vec<DetectorBox> = Vec::new();
        if config.enable_exact {
            detectors.push(Box::new(ExactDetector::new()));
        }
        if config.enable_semantic {
            detectors.push(Box::new(HistoryDetector::new()));
            let sidecar = SemanticSidecarClient::default();
            detectors.push(Box::new(SemanticDetector::new(sidecar)));
        }
        detectors.extend(extra_detectors);

        let execution_id = Uuid::new_v4();
        let pipeline_config = PipelineConfig {
            semantic_enabled: config.enable_semantic,
            exact_enabled: config.enable_exact,
            history_enabled: config.enable_semantic,
            rule_enabled: config.enable_rules,
            semantic_model: None,
        };
        let pipeline = ObservationPipeline::new(execution_id, detectors, pipeline_config);
        let history_store = Box::new(InMemoryHistoryStore::new(config.history_size));

        Ok(Self {
            config,
            pipeline,
            policy_engine,
            history_store,
            metrics: RecoveryMetrics::new(execution_id),
        })
    }

    pub fn from_yaml(policy_yaml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new(OrchestratorConfig::default(), policy_yaml, Vec::new())
    }

    pub fn process<'a>(
        &'a mut self,
        event: ToolCallEvent,
        capabilities: &'a AdapterCapabilities,
    ) -> (Option<Intervention>, Option<DetectionReport>) {
        self.process_inner(event, capabilities)
    }

    pub fn process_with_snapshot<'a>(
        &'a mut self,
        event: ToolCallEvent,
        capabilities: &'a AdapterCapabilities,
    ) -> (Option<Intervention>, Option<DetectionReport>) {
        let event_for_snapshot = event.clone();
        let (intervention, report) = self.process_inner(event, capabilities);
        (
            intervention,
            report.map(|r| DetectionReport {
                event: event_for_snapshot,
                ..r
            }),
        )
    }

    fn process_inner<'a>(
        &'a mut self,
        event: ToolCallEvent,
        capabilities: &'a AdapterCapabilities,
    ) -> (Option<Intervention>, Option<DetectionReport>) {
        let history = self.history_store.get_recent();
        let report = self.pipeline.observe(event.clone(), &history);
        let mut intervention = None;

        let has_errors = report.detections.iter().any(|d| {
            matches!(d, deadband_observation::detection::Detection::ErrorPattern { .. })
        });
        let effective_threshold = if has_errors && self.config.exact_threshold > 2 {
            self.config.exact_threshold - 1
        } else {
            self.config.exact_threshold
        };

        if !report.detections.is_empty() {
            let context = ExecutionContext {
                report: &report,
                history: &history,
                metrics: &self.metrics,

                config: &OrchestratorConfig {
                    exact_threshold: effective_threshold,
                    ..self.config.clone()
                },
                capabilities,
            };
            if let Some(raw) = self.policy_engine.evaluate(&context) {
                let (downgraded, _) = capabilities.downgrade(&raw);
                intervention = Some(downgraded);
            }
        }

        self.history_store.push(event.clone());

        (
            intervention,
            if report.detections.is_empty() { None } else { Some(report) },
        )
    }

    pub fn metrics(&self) -> &RecoveryMetrics {
        &self.metrics
    }

    pub fn policy_count(&self) -> usize {
        self.policy_engine.policy_count()
    }

    pub fn detector_count(&self) -> usize {
        self.pipeline.detector_count()
    }

    pub fn history(&self) -> Vec<ToolCallEvent> {
        self.history_store.get_recent()
    }

    pub fn record_intervention_outcome(
        &mut self,
        report: &DetectionReport,
        outcome: crate::intervention::InterventionOutcome,
        downgraded_from: Option<Intervention>,
    ) {
        self.metrics.record_event(MetricEvent::InterventionApplied {
            execution_id: self.metrics.execution_id,
            timestamp: report.metadata.timestamp,
            detection_kinds: report
                .detections
                .iter()
                .map(|d| d.kind().to_string())
                .collect(),
            outcome,
            downgraded_from,
        });
    }
}

#[cfg(test)]
mod tests {
    use deadband_observation::event::ErrorKind;
    use super::*;
    use serde_json::json;

    fn policy_yaml() -> &'static str {
        r#"
policies:
  - name: "repeat_abort"
    when:
      count:
        ExactRepeat: 3
    do:
      type: "Abort"
      params:
        reason: "detected loop"
        "#
    }

    fn make_event(name: &str, args: serde_json::Value) -> ToolCallEvent {
        ToolCallEvent::started("test", 0, name, args)
    }

    #[test]
    fn test_orchestrator_no_intervention() {
        let mut orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let event = make_event("search", json!({"q": "hello"}));
        let (intervention, _report) = orch.process(event, &AdapterCapabilities::default());
        assert!(intervention.is_none());
    }

    #[test]
    fn test_orchestrator_detects_repeat_loop() {
        let mut orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let event = make_event("search", json!({"q": "hello"}));
        assert!(orch.process(event.clone(), &AdapterCapabilities::default()).0.is_none());
        assert!(orch.process(event.clone(), &AdapterCapabilities::default()).0.is_none());
        let (intervention, _report) = orch.process(event, &AdapterCapabilities::default());
        assert!(intervention.is_some());
        assert!(intervention.unwrap().is_abort());
    }

    #[test]
    fn test_orchestrator_detects_error_pattern() {
        let yaml = r#"
policies:
  - name: "retry_timeout"
    when:
      count:
        ErrorPattern: 3
    do:
      type: "Retry"
      params:
        delay_ms: 100
        "#;
        let mut orch = Orchestrator::from_yaml(yaml).unwrap();
        let failed = event_failed("api_call", ErrorKind::Timeout);
        assert!(orch.process(failed.clone(), &AdapterCapabilities::default()).0.is_none());
        assert!(orch.process(failed.clone(), &AdapterCapabilities::default()).0.is_none());
        let (intervention, _report) = orch.process(failed, &AdapterCapabilities::default());
        assert!(intervention.is_some());
        assert!(intervention.unwrap().is_retry());
    }

    fn event_failed(name: &str, kind: ErrorKind) -> ToolCallEvent {
        ToolCallEvent::failed("test", 0, name, json!({}), kind, "boom".into(), 10)
    }

    #[test]
    fn test_orchestrator_history_bounded() {
        let config = OrchestratorConfig {
            history_size: 5,
            ..Default::default()
        };
        let mut orch = Orchestrator::new(config, policy_yaml(), Vec::new()).unwrap();
        for i in 0..10 {
            let event = make_event("tool", json!({"i": i}));
            orch.process(event, &AdapterCapabilities::default());
        }
        assert_eq!(orch.history().len(), 5);
    }

    #[test]
    fn test_process_with_snapshot() {
        let mut orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let event = make_event("search", json!({"q": "x"}));
        let (intervention, snapshot) = orch.process_with_snapshot(event, &AdapterCapabilities::default());
        assert!(intervention.is_none());
        assert!(snapshot.is_some());
    }

    #[test]
    fn test_metrics_tracked_after_adapter_execute() {
        use crate::intervention::InterventionOutcome;
        let mut orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let event = make_event("search", json!({"q": "x"}));
        orch.process(event.clone(), &AdapterCapabilities::default());
        orch.process(event.clone(), &AdapterCapabilities::default());
        let (intervention, report) = orch.process(event, &AdapterCapabilities::default());

        if let (Some(report), Some(_intervention)) = (report, intervention) {
            orch.record_intervention_outcome(
                &report,
                InterventionOutcome::Applied,
                None,
            );
        }

        assert!(orch.metrics().intervention_count > 0);
    }
}
