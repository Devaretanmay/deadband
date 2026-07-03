use deadband_core::{
    AdapterCapabilities, Intervention, InterventionOutcome, Orchestrator, ToolCallEvent,
};
use deadband_core::DetectionReport;
use serde_json::Value;

pub struct LooplessCrewAIFlow {
    orchestrator: Orchestrator,
    step: u64,
}

impl LooplessCrewAIFlow {
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
    use deadband_core::Orchestrator;
    use serde_json::json;

    #[test]
    fn test_flow_intercept() {
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
        let mut flow = LooplessCrewAIFlow::new(orch);
        let args = json!({"x": 1});
        assert!(flow.intercept_tool_call("tool", args.clone(), "t1").is_none());
        let result = flow.intercept_tool_call("tool", args.clone(), "t1");
        assert!(result.is_some());
        assert!(result.unwrap().is_abort());
    }

    #[test]
    fn test_flow_execute_abort() {
        let mut flow =
            LooplessCrewAIFlow::new(Orchestrator::from_yaml("policies: []").unwrap());
        let outcome = flow.execute(Intervention::Abort { reason: "loop".into() }, None);
        assert_eq!(outcome, InterventionOutcome::Applied);
    }
}
