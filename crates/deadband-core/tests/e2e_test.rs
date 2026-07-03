use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use deadband_core::*;

struct MockSemanticDetector {
    trigger: Arc<AtomicBool>,
}

impl MockSemanticDetector {
    fn new(trigger: Arc<AtomicBool>) -> Self {
        Self { trigger }
    }
}

impl Detector for MockSemanticDetector {
    fn name(&self) -> &str {
        "mock_semantic"
    }

    fn detect(&self, event: &ToolCallEvent, _history: &[ToolCallEvent]) -> Option<Detection> {
        if self.trigger.load(Ordering::SeqCst) {
            Some(Detection::SemanticRepeat {
                tool: event.tool_name.clone(),
                similarity: 0.85,
            })
        } else {
            None
        }
    }
}

fn policy_yaml() -> &'static str {
    r#"
policies:
  - name: "abort_on_3_repeats"
    when:
      count:
        ExactRepeat: 3
    do:
      type: "Abort"
      params:
        reason: "Exact repeat loop detected"
  - name: "abort_on_semantic_loop"
    when:
      any:
        - SemanticRepeat
    do:
      type: "Abort"
      params:
        reason: "Semantic loop detected"
"#
}

fn call_tool(
    orch: &mut Orchestrator,
    tool: &str,
    args: serde_json::Value,
    step: u64,
) -> Option<Intervention> {
    let event = ToolCallEvent::started("e2e-test", step, tool, args);
    let (intervention, report) = orch.process(event, &AdapterCapabilities::default());
    if let (Some(report), Some(_intervention)) = (report, intervention.clone()) {
        orch.record_intervention_outcome(
            &report,
            deadband_core::InterventionOutcome::Applied,
            None,
        );
    }
    intervention
}

#[test]
fn e2e_exact_repeat_detection_and_abort() {
    let trigger = Arc::new(AtomicBool::new(false));

    let config = OrchestratorConfig {
        enable_semantic: true,
        exact_threshold: 1,
        error_threshold: 3,
        ..Default::default()
    };

    let mut orch = Orchestrator::new(config, policy_yaml(), vec![
        Box::new(MockSemanticDetector::new(trigger)),
    ])
    .unwrap();

    let args = serde_json::json!({"query": "SELECT * FROM users"});

    // Call 1: first time, no loop
    assert!(call_tool(&mut orch, "query_db", args.clone(), 0).is_none());

    // Call 2: second time, no loop yet (threshold in policy is 3)
    assert!(call_tool(&mut orch, "query_db", args.clone(), 1).is_none());

    // Call 3: this should trigger ExactRepeat → Abort
    let intervention = call_tool(&mut orch, "query_db", args.clone(), 2);
    assert!(intervention.is_some(), "Should abort on 3rd exact repeat");

    let i = intervention.unwrap();
    assert!(i.is_abort(), "Should be abort intervention");
    assert_eq!(i.reason(), Some("Exact repeat loop detected"));
}

#[test]
fn e2e_semantic_loop_detection_with_different_tools() {
    let trigger = Arc::new(AtomicBool::new(false));

    let config = OrchestratorConfig {
        enable_semantic: true,
        exact_threshold: 5, // High threshold so exact doesn't fire
        ..Default::default()
    };

    let mut orch = Orchestrator::new(config, policy_yaml(), vec![
        Box::new(MockSemanticDetector::new(trigger.clone())),
    ])
    .unwrap();

    let args = serde_json::json!({"line": 42});

    // Different tool names but semantically same intent
    // First call passes through
    assert!(call_tool(&mut orch, "delete_line", args.clone(), 0).is_none());

    // Enable loop detection for second call
    trigger.store(true, Ordering::SeqCst);

    // Second call: semantically similar → semantic detector fires
    let intervention = call_tool(&mut orch, "remove_line", args.clone(), 1);
    assert!(intervention.is_some(), "Should detect semantic loop");
    assert!(intervention.unwrap().is_abort());
}

#[test]
fn e2e_metrics_after_intervention() {
    let trigger = Arc::new(AtomicBool::new(true));

    let config = OrchestratorConfig {
        enable_semantic: true,
        exact_threshold: 1,
        ..Default::default()
    };

    let mut orch = Orchestrator::new(config, policy_yaml(), vec![
        Box::new(MockSemanticDetector::new(trigger)),
    ])
    .unwrap();

    // Simulate several tool calls and one intervention
    for i in 0..5 {
        let args = serde_json::json!({"i": i});
        call_tool(&mut orch, "generic_tool", args, i as u64);
    }

    let metrics = orch.metrics();
    assert!(metrics.intervention_count > 0, "Should have recorded at least one intervention");
    assert!(metrics.total_detections() > 0, "Should have detections recorded");
    assert!(!metrics.detection_breakdown.is_empty(), "Should have detection breakdown");
}
