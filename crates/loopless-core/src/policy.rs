use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use loopless_observation::detection::Detection;
use crate::context::ExecutionContext;
use crate::intervention::{Intervention, PromptPosition};

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("YAML parse error: {0}")]
    ParseError(String),
    #[error("Unknown action type: {0}")]
    UnknownAction(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PolicyConfig {
    pub policies: Vec<PolicyEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PolicyEntry {
    pub name: String,
    #[serde(rename = "when")]
    pub condition: ConditionConfig,
    #[serde(rename = "do")]
    pub action: ActionConfig,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConditionConfig {
    pub any: Option<Vec<String>>,
    pub all: Option<Vec<String>>,
    pub count: Option<HashMap<String, u32>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActionConfig {
    #[serde(rename = "type")]
    pub action_type: String,
    pub params: Option<HashMap<String, Value>>,
}


pub struct PolicyEngine {
    policies: Vec<PolicyEntry>,
}

impl PolicyEngine {
    pub fn new(yaml_str: &str) -> Result<Self, PolicyError> {
        let config: PolicyConfig =
            serde_yaml::from_str(yaml_str).map_err(|e| PolicyError::ParseError(e.to_string()))?;

        for policy in &config.policies {
            validate_action(&policy.action)?;
        }

        Ok(Self {
            policies: config.policies,
        })
    }

    pub fn evaluate(&self, context: &ExecutionContext) -> Option<Intervention> {
        let mut candidates: Vec<&PolicyEntry> = self
            .policies
            .iter()
            .filter(|p| condition_matches(&p.condition, context))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by_key(|p| -p.priority);
        Some(action_to_intervention(&candidates[0].action))
    }

    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }
}

fn validate_action(action: &ActionConfig) -> Result<(), PolicyError> {
    match action.action_type.as_str() {
        "Retry" | "Backoff" | "ReplaceTool" | "InjectPrompt" | "Abort" => Ok(()),
        other => Err(PolicyError::UnknownAction(other.into())),
    }
}

fn condition_matches(cond: &ConditionConfig, ctx: &ExecutionContext) -> bool {
    let to_snake = |s: &str| -> String {
        match s.to_lowercase().as_str() {
            "exactrepeat" => "exact_repeat".to_string(),
            "semanticrepeat" => "semantic_repeat".to_string(),
            "ruleviolation" => "rule_violation".to_string(),
            "errorpattern" => "error_pattern".to_string(),
            "budgetexceeded" => "budget_exceeded".to_string(),
            _ => s.to_string(),
        }
    };

    if let Some(kinds) = &cond.any {
        return kinds.iter().any(|k| {
            let k_snake = to_snake(k);
            ctx.report.detections.iter().any(|d| d.kind() == k_snake)
        });
    }

    if let Some(kinds) = &cond.all {
        return kinds.iter().all(|k| {
            let k_snake = to_snake(k);
            ctx.report.detections.iter().any(|d| d.kind() == k_snake)
        });
    }

    if let Some(map) = &cond.count {
        return map.iter().all(|(kind, threshold)| {
            let k_snake = to_snake(kind);
            ctx.report.detections.iter().any(|d| {
                if d.kind() != k_snake {
                    return false;
                }
                match d {
                    Detection::ExactRepeat { count, .. } => count >= threshold,
                    Detection::ErrorPattern { count, .. } => count >= threshold,
                    _ => false,
                }
            })
        });
    }

    false
}

fn action_to_intervention(action: &ActionConfig) -> Intervention {
    match action.action_type.as_str() {
        "Retry" => {
            let delay = action
                .params
                .as_ref()
                .and_then(|p| p.get("delay_ms"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Intervention::Retry { delay_ms: delay }
        }
        "Backoff" => {
            let base = action
                .params
                .as_ref()
                .and_then(|p| p.get("base_ms"))
                .and_then(|v| v.as_u64())
                .unwrap_or(500);
            Intervention::Backoff {
                base_ms: base,
                attempt: 0,
            }
        }
        "ReplaceTool" => {
            let original = action
                .params
                .as_ref()
                .and_then(|p| p.get("original"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let replacement = action
                .params
                .as_ref()
                .and_then(|p| p.get("replacement"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Intervention::ReplaceTool {
                original,
                replacement,
            }
        }
        "InjectPrompt" => {
            let content = action
                .params
                .as_ref()
                .and_then(|p| p.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let position = action
                .params
                .as_ref()
                .and_then(|p| p.get("position"))
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "ReplaceLast" => PromptPosition::ReplaceLast,
                    "AfterTool" => PromptPosition::AfterTool,
                    _ => PromptPosition::BeforeNext,
                })
                .unwrap_or(PromptPosition::BeforeNext);
            Intervention::InjectPrompt { content, position }
        }
        "Abort" => {
            let reason = action
                .params
                .as_ref()
                .and_then(|p| p.get("reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("Aborted by policy")
                .to_string();
            Intervention::Abort { reason }
        }
        _ => Intervention::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopless_observation::detection::Detection;
    use loopless_observation::event::{ErrorKind, ToolCallEvent};
    use loopless_observation::report::{DetectionReport, ObservationMetadata};
    use crate::orchestrator::OrchestratorConfig;
    use crate::metrics::RecoveryMetrics;
    use crate::intervention::AdapterCapabilities;
    use uuid::Uuid;
    use serde_json::json;

    fn make_engine(yaml: &str) -> PolicyEngine {
        PolicyEngine::new(yaml).unwrap()
    }

    fn make_report(detections: Vec<Detection>) -> DetectionReport {
        let event = ToolCallEvent::started("test", 0, "x", json!({}));
        DetectionReport::new(
            Uuid::new_v4(),
            event,
            detections,
            ObservationMetadata::default(),
        )
    }

    #[test]
    fn test_valid_yaml_parses() {
        let yaml = r#"
policies:
  - name: "test"
    when:
      any:
        - ExactRepeat
    do:
      type: "Abort"
      params:
        reason: "test"
"#;
        let engine = make_engine(yaml);
        assert_eq!(engine.policy_count(), 1);
    }

    #[test]
    fn test_any_matches() {
        let yaml = r#"
policies:
  - name: "repeat_abort"
    when:
      any:
        - ExactRepeat
        - ErrorPattern
    do:
      type: "Abort"
"#;
        let engine = make_engine(yaml);
        let report = make_report(vec![Detection::ExactRepeat {
            tool: "x".into(),
            count: 3,
        }]);
        let config = OrchestratorConfig::default();
        let metrics = RecoveryMetrics::new(Uuid::new_v4());
        let ctx = ExecutionContext {
            report: &report,
            history: &[],
            metrics: &metrics,
            config: &config,
            capabilities: &AdapterCapabilities::default(),
        };
        let intervention = engine.evaluate(&ctx);
        assert!(intervention.is_some());
        assert!(intervention.unwrap().is_abort());
    }

    #[test]
    fn test_all_matches() {
        let yaml = r#"
policies:
  - name: "retry_timeout"
    when:
      all:
        - ErrorPattern
        - BudgetExceeded
    do:
      type: "Retry"
"#;
        let engine = make_engine(yaml);
        
        // Only one detection - should fail
        let report_fail = make_report(vec![Detection::ErrorPattern {
            kind: ErrorKind::Timeout,
            count: 1,
        }]);
        let config = OrchestratorConfig::default();
        let metrics = RecoveryMetrics::new(Uuid::new_v4());
        let ctx_fail = ExecutionContext {
            report: &report_fail,
            history: &[],
            metrics: &metrics,
            config: &config,
            capabilities: &AdapterCapabilities::default(),
        };
        assert!(engine.evaluate(&ctx_fail).is_none());

        // Both detections - should match
        let report_pass = make_report(vec![
            Detection::ErrorPattern {
                kind: ErrorKind::Timeout,
                count: 1,
            },
            Detection::BudgetExceeded {
                budget: 10,
                spent: 10,
            }
        ]);
        let ctx_pass = ExecutionContext {
            report: &report_pass,
            history: &[],
            metrics: &metrics,
            config: &config,
            capabilities: &AdapterCapabilities::default(),
        };
        assert!(engine.evaluate(&ctx_pass).is_some());
    }

    #[test]
    fn test_count_matches() {
        let yaml = r#"
policies:
  - name: "nudge"
    when:
      count:
        ExactRepeat: 3
    do:
      type: "InjectPrompt"
"#;
        let engine = make_engine(yaml);
        
        // Count is 2 - should fail
        let report_fail = make_report(vec![Detection::ExactRepeat {
            tool: "x".into(),
            count: 2,
        }]);
        let config = OrchestratorConfig::default();
        let metrics = RecoveryMetrics::new(Uuid::new_v4());
        let ctx_fail = ExecutionContext {
            report: &report_fail,
            history: &[],
            metrics: &metrics,
            config: &config,
            capabilities: &AdapterCapabilities::default(),
        };
        assert!(engine.evaluate(&ctx_fail).is_none());

        // Count is 3 - should match
        let report_pass = make_report(vec![Detection::ExactRepeat {
            tool: "x".into(),
            count: 3,
        }]);
        let ctx_pass = ExecutionContext {
            report: &report_pass,
            history: &[],
            metrics: &metrics,
            config: &config,
            capabilities: &AdapterCapabilities::default(),
        };
        let intervention = engine.evaluate(&ctx_pass).unwrap();
        assert!(intervention.is_inject_prompt());
    }
}
