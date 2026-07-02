use loopless_core::{
    AdapterCapabilities, Intervention, InterventionOutcome, Orchestrator, ToolCallEvent,
};
use loopless_core::DetectionReport;
use serde_json::Value;

pub struct LooplessOpenAIToolWrapper {
    orchestrator: Orchestrator,
    step: u64,
}

impl LooplessOpenAIToolWrapper {
    pub fn new(orchestrator: Orchestrator) -> Self {
        Self {
            orchestrator,
            step: 0,
        }
    }

    pub fn intercept_tool_call(
        &mut self,
        tool_name: &str,
        arguments: Value,
        thread_id: &str,
    ) -> Option<Intervention> {
        let event = ToolCallEvent::started(thread_id, self.step, tool_name, arguments);
        self.step += 1;
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
            Intervention::Abort { .. } => {
                self.step = 0;
                InterventionOutcome::Applied
            }
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

    #[test]
    fn test_wrapper_intercept() {
        let orch = Orchestrator::from_yaml(
            r#"
policies:
  - name: "abort"
    when:
      count:
        ExactRepeat: 2
    do:
      type: "Abort"
      params:
        reason: "loop"
"#,
        )
        .unwrap();
        let mut wrapper = LooplessOpenAIToolWrapper::new(orch);
        let args = json!({"q": "test"});
        assert!(wrapper.intercept_tool_call("search", args.clone(), "t1").is_none());
        let result = wrapper.intercept_tool_call("search", args.clone(), "t1");
        assert!(result.is_some());
    }

    #[test]
    fn test_wrapper_execute_abort() {
        let mut wrapper =
            LooplessOpenAIToolWrapper::new(Orchestrator::from_yaml("policies: []").unwrap());
        let outcome = wrapper.execute(Intervention::Abort { reason: "loop".into() }, None);
        assert_eq!(outcome, InterventionOutcome::Applied);
    }
}
