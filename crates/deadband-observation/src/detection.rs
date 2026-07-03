
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::{ErrorKind, ToolCallEvent};
use crate::canonical::{auto_infer_volatile_fields, strip_volatile_fields as canonical_strip};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Detection {
    ExactRepeat {
        tool: String,
        count: u32,
    },
    SemanticRepeat {
        tool: String,
        similarity: f32,
    },
    RuleViolation {
        rule: String,
        detail: String,
    },
    ErrorPattern {
        kind: ErrorKind,
        count: u32,
    },
}

impl Detection {
    pub fn kind(&self) -> &'static str {
        match self {
            Detection::ExactRepeat { .. } => "exact_repeat",
            Detection::SemanticRepeat { .. } => "semantic_repeat",
            Detection::RuleViolation { .. } => "rule_violation",
            Detection::ErrorPattern { .. } => "error_pattern",

        }
    }
    pub fn intrinsic_severity_and_confidence(&self) -> (crate::report::Severity, f32) {
        use crate::report::Severity;
        match self {
            Detection::ExactRepeat { count, .. } => {
                if *count > 5 {
                    (Severity::High, 1.0)
                } else if *count > 2 {
                    (Severity::Medium, 1.0)
                } else {
                    (Severity::Low, 1.0)
                }
            },
            Detection::SemanticRepeat { similarity, .. } => {
                (Severity::Medium, *similarity)
            },
            Detection::RuleViolation { .. } => (Severity::High, 1.0),
            Detection::ErrorPattern { count, .. } => {
                if *count > 5 {
                    (Severity::High, 1.0)
                } else {
                    (Severity::Medium, 1.0)
                }
            },

        }
    }
}

pub trait Detector: Send {
    fn name(&self) -> &str;
    fn detect(&self, event: &ToolCallEvent, history: &[ToolCallEvent]) -> Option<Detection>;
}

pub struct ExactDetector {
    ignore_args: bool,
    volatile_fields: Vec<String>,
    enable_auto_inference: bool,
}

impl ExactDetector {
    pub fn new() -> Self {
        Self {
            ignore_args: false,
            volatile_fields: Vec::new(),
            enable_auto_inference: true,
        }
    }

    pub fn with_ignore_args(mut self, ignore: bool) -> Self {
        self.ignore_args = ignore;
        self
    }

    pub fn with_volatile_fields(mut self, fields: Vec<String>) -> Self {
        self.volatile_fields = fields;
        self
    }




    pub fn with_auto_inference(mut self, enable: bool) -> Self {
        self.enable_auto_inference = enable;
        self
    }
}

impl Detector for ExactDetector {
    fn name(&self) -> &str {
        "exact"
    }

    fn detect(&self, event: &ToolCallEvent, history: &[ToolCallEvent]) -> Option<Detection> {
        let mut all_paths: Vec<String> = self.volatile_fields.iter()
            .map(|f| format!(".{}", f))
            .collect();
        if self.enable_auto_inference {
            let history_refs: Vec<(&str, &Value)> = history.iter()
                .map(|h| (h.tool_name.as_str(), &h.arguments))
                .collect();
            let auto_fields = auto_infer_volatile_fields(
                &event.arguments, &history_refs, &event.tool_name, 2,
            );
            for f in &auto_fields {
                all_paths.push(format!(".{}", f));
            }
        }

        let mut event_val = event.arguments.clone();
        if !all_paths.is_empty() {
            canonical_strip(&mut event_val, &all_paths);
        }

        let count = 1 + history.iter()
            .filter(|h| {
                h.tool_name == event.tool_name
                    && (self.ignore_args || {
                        let mut h_val = h.arguments.clone();
                        if !all_paths.is_empty() {
                            canonical_strip(&mut h_val, &all_paths);
                        }
                        h_val == event_val
                    })
            })
            .count();

        Some(Detection::ExactRepeat {
            tool: event.tool_name.clone(),
            count: count as u32,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SidecarShadowMetrics {

    pub sidecar_unavailable_count: u64,

    pub shadow_loops_missed: u64,

    pub shadow_mode_active: bool,
}

impl SidecarShadowMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_unavailable(&mut self) {
        self.sidecar_unavailable_count += 1;
        self.shadow_mode_active = true;
    }

    pub fn record_missed_loop(&mut self) {
        self.shadow_loops_missed += 1;
    }

    pub fn reset(&mut self) {
        self.sidecar_unavailable_count = 0;
        self.shadow_loops_missed = 0;
        self.shadow_mode_active = false;
    }
}

pub struct SemanticSidecarClient {
    base_url: String,
    threshold: f32,
    timeout_ms: u64,
    shadow_metrics: Mutex<SidecarShadowMetrics>,
}

impl SemanticSidecarClient {
    pub fn new(base_url: impl Into<String>, threshold: f32) -> Self {
        Self {
            base_url: base_url.into(),
            threshold,
            timeout_ms: 500,
            shadow_metrics: Mutex::new(SidecarShadowMetrics::new()),
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }


    pub fn shadow_metrics(&self) -> SidecarShadowMetrics {
        self.shadow_metrics.lock().unwrap().clone()
    }

    fn call_sidecar(&self, session_id: &str, tool_call: &str, args: &str) -> Option<bool> {
        let url = format!("{}/analyze", self.base_url);
        let payload = serde_json::json!({
            "session_id": session_id,
            "tool_call": tool_call,
            "llm_error_response": args,
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build()
            .ok()?;

        let resp = client.post(&url).json(&payload).send();
        match resp {
            Ok(resp) => {
                if !resp.status().is_success() {

                    tracing::warn!(
                        "Semantic sidecar returned status {} — entering shadow mode (exact detection fallback)",
                        resp.status()
                    );
                    self.shadow_metrics.lock().unwrap().record_unavailable();
                    return None;
                }
                let body: Value = resp.json().ok()?;
                let loop_detected = body.get("loop_detected").and_then(|v| v.as_bool());


                {
                    let mut metrics = self.shadow_metrics.lock().unwrap();
                    if metrics.shadow_mode_active {
                        tracing::info!("Semantic sidecar recovered — shadow mode deactivated");
                        metrics.reset();
                    }
                }

                loop_detected
            }
            Err(e) => {

                let mut metrics = self.shadow_metrics.lock().unwrap();
                if !metrics.shadow_mode_active {
                    tracing::warn!(
                        "Semantic sidecar unreachable ({}: {}) — entering shadow mode, using exact detection only. {} loops would have been caught if sidecar were active.",
                        self.base_url, e, metrics.shadow_loops_missed
                    );
                }
                metrics.record_unavailable();
                None
            }
        }
    }
}

impl Default for SemanticSidecarClient {
    fn default() -> Self {
        Self::new("http://localhost:8081", 0.85)
    }
}

pub struct SemanticDetector {
    sidecar: SemanticSidecarClient,
}

impl SemanticDetector {
    pub fn new(sidecar: SemanticSidecarClient) -> Self {
        Self { sidecar }
    }

    pub fn is_sidecar_running(&self) -> bool {
        let url = format!("{}/", self.sidecar.base_url);
        reqwest::blocking::get(&url).is_ok()
    }


    pub fn shadow_metrics(&self) -> SidecarShadowMetrics {
        self.sidecar.shadow_metrics()
    }


    pub fn is_shadow_mode(&self) -> bool {
        self.sidecar.shadow_metrics.lock().unwrap().shadow_mode_active
    }
}

impl Detector for SemanticDetector {
    fn name(&self) -> &str {
        "semantic"
    }

    fn detect(&self, event: &ToolCallEvent, _history: &[ToolCallEvent]) -> Option<Detection> {
        let args_str = serde_json::to_string(&event.arguments).unwrap_or_default();
        let loop_detected = self
            .sidecar
            .call_sidecar(&event.thread_id, &event.tool_name, &args_str);

        match loop_detected {
            Some(true) => {

                Some(Detection::SemanticRepeat {
                    tool: event.tool_name.clone(),
                    similarity: self.sidecar.threshold,
                })
            }
            Some(false) => {

                None
            }
            None => {

                None
            }
        }
    }
}

pub struct RuleDetector {
    rules: Vec<CompiledRule>,
}

impl RuleDetector {
    pub fn new(rules: Vec<CompiledRule>) -> Self {
        Self { rules }
    }
}

impl Detector for RuleDetector {
    fn name(&self) -> &str {
        "rule"
    }

    fn detect(&self, event: &ToolCallEvent, _history: &[ToolCallEvent]) -> Option<Detection> {
        for rule in &self.rules {
            if let Err(detail) = rule.matches(event) {
                return Some(Detection::RuleViolation {
                    rule: rule.name(),
                    detail,
                });
            }
        }
        None
    }
}

pub enum CompiledRule {
    Regex {
        name: String,
        pattern: regex::Regex,
    },
    Exact {
        name: String,
        value: String,
    },
    JsonSchema {
        name: String,
        required: Vec<String>,
        schema_type: String,
    },
    ToolName {
        name: String,
        blocked: Vec<String>,
    },
}

impl CompiledRule {
    pub fn name(&self) -> String {
        match self {
            CompiledRule::Regex { name, .. } => name.clone(),
            CompiledRule::Exact { name, .. } => name.clone(),
            CompiledRule::JsonSchema { name, .. } => name.clone(),
            CompiledRule::ToolName { name, .. } => name.clone(),
        }
    }

    pub fn matches(&self, event: &ToolCallEvent) -> Result<(), String> {
        match self {

            CompiledRule::Regex { pattern, .. } => {
                let mc_rule = microloop::engine::CompiledRule::Regex(pattern.clone());
                let args_str = serde_json::to_string(&event.arguments).unwrap_or_default();
                mc_rule.matches(&args_str).map_err(|e| format!("Regex rule: {}", e))
            }
            CompiledRule::Exact { value, .. } => {
                let mc_rule = microloop::engine::CompiledRule::Exact(value.clone());
                let args_str = serde_json::to_string(&event.arguments).unwrap_or_default();
                mc_rule.matches(&args_str).map_err(|e| format!("Exact match: {}", e))
            }
            CompiledRule::JsonSchema {
                required,
                schema_type,
                ..
            } => {
                let mc_rule = microloop::engine::CompiledRule::JsonSchema {
                    required: required.clone(),
                    schema_type: schema_type.clone(),
                };
                let args_str = serde_json::to_string(&event.arguments).unwrap_or_default();
                mc_rule.matches(&args_str).map_err(|e| format!("JSON schema: {}", e))
            }

            CompiledRule::ToolName { blocked, .. } => {
                if blocked.contains(&event.tool_name) {
                    Err(format!("Tool blocked: {}", event.tool_name))
                } else {
                    Ok(())
                }
            }
        }
    }
}

pub struct HistoryDetector;

impl HistoryDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for HistoryDetector {
    fn name(&self) -> &str {
        "history"
    }

    fn detect(&self, event: &ToolCallEvent, history: &[ToolCallEvent]) -> Option<Detection> {
        let kind = event.error_kind()?;
        let count = history
            .iter()
            .filter_map(|h| h.error_kind())
            .filter(|k| *k == kind)
            .count() as u32
            + 1;

        Some(Detection::ErrorPattern {
            kind,
            count,
        })
    }
}

pub type DetectorBox = Box<dyn Detector>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::ToolCallEvent;
    use serde_json::json;

    fn make_tool(name: &str, args: Value) -> ToolCallEvent {
        ToolCallEvent::started("test", 0, name, args)
    }

    fn make_failed(name: &str, kind: ErrorKind) -> ToolCallEvent {
        ToolCallEvent::failed("test", 0, name, json!({}), kind, "error".into(), 0)
    }

    #[test]
    fn test_exact_detector_no_loop() {
        let detector = ExactDetector::new();
        let event = make_tool("get_data", json!({"id": 1}));
        let history = vec![
            make_tool("get_data", json!({"id": 2})),
            make_tool("get_data", json!({"id": 3})),
        ];
        let detection = detector.detect(&event, &history);
        assert!(detection.is_some());
        assert_eq!(detection.unwrap().kind(), "exact_repeat");
    }

    #[test]
    fn test_exact_detector_loop() {
        let detector = ExactDetector::new();
        let event = make_tool("get_data", json!({"id": 1}));
        let history = vec![
            make_tool("get_data", json!({"id": 1})),
            make_tool("get_data", json!({"id": 1})),
        ];
        let detection = detector.detect(&event, &history);
        assert!(detection.is_some());
        assert_eq!(detection.unwrap().kind(), "exact_repeat");
    }

    #[test]
    fn test_exact_detector_different_tools() {
        let detector = ExactDetector::new();
        let event = make_tool("search", json!({"q": "x"}));
        let history = vec![make_tool("compute", json!({"q": "x"}))];
        let detection = detector.detect(&event, &history);
        assert!(detection.is_some());
        if let Detection::ExactRepeat { count, .. } = detection.unwrap() {
            assert_eq!(count, 1);
        } else {
            panic!("Expected ExactRepeat");
        }
    }

    #[test]
    fn test_history_detector() {
        let detector = HistoryDetector::new();
        let event = make_failed("api_call", ErrorKind::Timeout);
        let history = vec![
            make_failed("api_call", ErrorKind::Timeout),
            make_failed("api_call", ErrorKind::Timeout),
        ];
        let detection = detector.detect(&event, &history);
        assert!(detection.is_some());
        let d = detection.unwrap();
        assert_eq!(d.kind(), "error_pattern");
    }

    #[test]
    fn test_history_detector_different_errors() {
        let detector = HistoryDetector::new();
        let event = make_failed("api_call", ErrorKind::Permission);
        let history = vec![make_failed("api_call", ErrorKind::Timeout)];
        let detection = detector.detect(&event, &history);
        assert!(detection.is_some());
        if let Detection::ErrorPattern { count, .. } = detection.unwrap() {
            assert_eq!(count, 1);
        } else {
            panic!("Expected ErrorPattern");
        }
    }

    #[test]
    fn test_rule_tool_name_blocked() {
        let rule = CompiledRule::ToolName {
            name: "no_delete".into(),
            blocked: vec!["delete_file".into()],
        };
        let event = make_tool("delete_file", json!({}));
        assert!(rule.matches(&event).is_err());
        let event = make_tool("read_file", json!({}));
        assert!(rule.matches(&event).is_ok());
    }

    #[test]
    fn test_rule_exact_match_ok() {
        let rule = CompiledRule::Exact {
            name: "exact_args".into(),
            value: r#"{"key":"value"}"#.into(),
        };
        let event = make_tool("x", json!({"key": "value"}));
        assert!(rule.matches(&event).is_ok());
    }

    #[test]
    fn test_rule_exact_no_match() {
        let rule = CompiledRule::Exact {
            name: "exact_args".into(),
            value: r#"{"key":"other"}"#.into(),
        };
        let event = make_tool("x", json!({"key": "value"}));
        assert!(rule.matches(&event).is_err());
    }
}
