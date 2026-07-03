use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use deadband_core::{
    AdapterCapabilities, Detection, Detector, ToolCallEvent,
};

#[derive(Clone)]
struct MockSemanticDetector {
    trigger_loop: Arc<AtomicBool>,
    captured: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl MockSemanticDetector {
    fn new(trigger_loop: Arc<AtomicBool>) -> Self {
        Self {
            trigger_loop,
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn captured(&self) -> Vec<(String, String, String)> {
        self.captured.lock().unwrap().clone()
    }
}

impl Detector for MockSemanticDetector {
    fn name(&self) -> &str {
        "mock_semantic"
    }

    fn detect(&self, event: &ToolCallEvent, _history: &[ToolCallEvent]) -> Option<Detection> {
        self.captured.lock().unwrap().push((
            event.thread_id.clone(),
            event.tool_name.clone(),
            event.arguments.to_string(),
        ));

        if self.trigger_loop.load(Ordering::SeqCst) {
            Some(Detection::SemanticRepeat {
                tool: event.tool_name.clone(),
                similarity: 0.85,
            })
        } else {
            None
        }
    }
}

#[test]
fn test_semantic_detector_no_loop_first_call() {
    let trigger = Arc::new(AtomicBool::new(false));
    let detector = MockSemanticDetector::new(trigger);

    let event = ToolCallEvent::started("test-session", 0, "search", serde_json::json!({"q": "hello"}));
    let result = detector.detect(&event, &[]);
    assert!(result.is_none(), "First call should not trigger detection");
}

#[test]
fn test_semantic_detector_detects_loop() {
    let trigger = Arc::new(AtomicBool::new(true));
    let detector = MockSemanticDetector::new(trigger);

    let event = ToolCallEvent::started("test-session", 0, "search", serde_json::json!({"q": "hello"}));
    let result = detector.detect(&event, &[]);
    assert!(result.is_some(), "Should detect semantic loop when sidecar says so");

    if let Some(Detection::SemanticRepeat { tool, similarity }) = result {
        assert_eq!(tool, "search");
        assert!(similarity >= 0.85);
    } else {
        panic!("Expected SemanticRepeat detection");
    }
}

#[test]
fn test_semantic_detector_sidecar_down() {
    let trigger = Arc::new(AtomicBool::new(false));
    let detector = MockSemanticDetector::new(trigger);

    let event = ToolCallEvent::started("test-session", 0, "search", serde_json::json!({"q": "hello"}));
    let result = detector.detect(&event, &[]);
    assert!(result.is_none(), "Should gracefully handle detector being disabled");
}

#[test]
fn test_semantic_detector_sends_correct_payload() {
    let trigger = Arc::new(AtomicBool::new(true));
    let detector = MockSemanticDetector::new(trigger);

    let event = ToolCallEvent::started("session-42", 0, "delete_line", serde_json::json!({"line": 5}));
    let result = detector.detect(&event, &[]);
    assert!(result.is_some(), "Detector should receive correct event context");
    assert_eq!(
        detector.captured(),
        vec![(
            "session-42".to_string(),
            "delete_line".to_string(),
            serde_json::json!({"line": 5}).to_string(),
        )]
    );
}

#[test]
fn test_orchestrator_with_semantic_detector() {
    use deadband_core::Orchestrator;

    let trigger = Arc::new(AtomicBool::new(true));
    let detector = MockSemanticDetector::new(trigger);

    let config = deadband_core::OrchestratorConfig {
        enable_semantic: true,
        ..Default::default()
    };

    let yaml = r#"
policies:
  - name: "abort_on_semantic_loop"
    when:
      any:
        - SemanticRepeat
    do:
      type: "Abort"
      params:
        reason: "semantic loop detected"
"#;

    let mut orch = Orchestrator::new(
        config,
        yaml,
        vec![Box::new(detector)],
    )
    .unwrap();

    let event = ToolCallEvent::started("test", 0, "delete", serde_json::json!({"id": 1}));
    let (intervention, _report) = orch.process(event, &AdapterCapabilities::default());
    assert!(intervention.is_some(), "Semantic loop should trigger abort");
    assert!(intervention.unwrap().is_abort());
}
