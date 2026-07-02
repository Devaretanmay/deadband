use loopless_core::{
    AdapterCapabilities, Intervention, InterventionOutcome, Orchestrator, ToolCallEvent,
};
use loopless_core::DetectionReport;
use serde_json::Value;

pub struct LooplessLangGraphMiddleware {
    orchestrator: Orchestrator,
}

impl LooplessLangGraphMiddleware {
    pub fn new(orchestrator: Orchestrator) -> Self {
        Self { orchestrator }
    }

    pub fn wrap_tool_call(
        &mut self,
        tool_name: &str,
        arguments: Value,
        thread_id: &str,
        step: u64,
    ) -> Option<Intervention> {
        let event = ToolCallEvent::started(thread_id, step, tool_name, arguments);
        let (intervention, _report) =
            self.orchestrator.process(event, &AdapterCapabilities::default());
        intervention
    }

    pub fn execute(
        &mut self,
        intervention: Intervention,
        _report: Option<DetectionReport>,
    ) -> InterventionOutcome {
        match intervention {
            Intervention::Continue => InterventionOutcome::Applied,
            Intervention::Abort { .. } => InterventionOutcome::Applied,
            Intervention::Retry { .. } | Intervention::Backoff { .. } => {
                InterventionOutcome::Applied
            }
            Intervention::ReplaceTool { .. } | Intervention::InjectPrompt { .. } => {
                InterventionOutcome::Unsupported
            }
            Intervention::Custom { .. } => InterventionOutcome::Unsupported,
        }
    }

    pub fn into_orchestrator(self) -> Orchestrator {
        self.orchestrator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopless_core::Orchestrator;
    use serde_json::json;

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
        reason: "detected loop"
"#
    }

    #[test]
    fn test_middleware_no_intervention() {
        let orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let mut mw = LooplessLangGraphMiddleware::new(orch);
        let result = mw.wrap_tool_call("search", json!({"q": "hi"}), "t1", 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_middleware_detects_loop() {
        let orch = Orchestrator::from_yaml(policy_yaml()).unwrap();
        let mut mw = LooplessLangGraphMiddleware::new(orch);
        let args = json!({"q": "hi"});
        assert!(mw.wrap_tool_call("search", args.clone(), "t1", 0).is_none());
        assert!(mw.wrap_tool_call("search", args.clone(), "t1", 1).is_none());
        let result = mw.wrap_tool_call("search", args.clone(), "t1", 2);
        assert!(result.is_some());
        assert!(result.unwrap().is_abort());
    }

    #[test]
    fn test_middleware_execute_abort() {
        let mut mw =
            LooplessLangGraphMiddleware::new(Orchestrator::from_yaml("policies: []").unwrap());
        let outcome = mw.execute(Intervention::Abort { reason: "loop".into() }, None);
        assert_eq!(outcome, InterventionOutcome::Applied);
    }
}
