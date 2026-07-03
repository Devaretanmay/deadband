
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InterventionOutcome {
    Applied,
    Rejected,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    pub retry: bool,
    pub replace_tool: bool,
    pub inject_prompt: bool,
    pub abort: bool,
    pub checkpoint_restore: bool,
    pub max_backoff_ms: u64,
}

impl Default for AdapterCapabilities {
    fn default() -> Self {
        Self {
            retry: true,
            replace_tool: false,
            inject_prompt: false,
            abort: true,
            checkpoint_restore: false,
            max_backoff_ms: 30_000,
        }
    }
}

impl AdapterCapabilities {
    pub fn supports(&self, intervention: &Intervention) -> bool {
        match intervention {
            Intervention::Continue => true,
            Intervention::Retry { .. } | Intervention::Backoff { .. } => self.retry,
            Intervention::ReplaceTool { .. } => self.replace_tool,
            Intervention::InjectPrompt { .. } => self.inject_prompt,
            Intervention::Abort { .. } => self.abort,
            Intervention::Custom { .. } => false,
        }
    }

    pub fn downgrade(
        &self,
        intervention: &Intervention,
    ) -> (Intervention, Option<InterventionOutcome>) {
        if self.supports(intervention) {
            return (intervention.clone(), None);
        }

        let fallback = match intervention {
            Intervention::InjectPrompt { content, position: _, prompt_id: _ } => {
                if self.retry {
                    (
                        Intervention::Retry { delay_ms: 0 },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else if self.abort {
                    (
                        Intervention::Abort {
                            reason: format!(
                                "inject_prompt unsupported, downgrading to abort: {}",
                                content
                            ),
                        },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else {
                    (Intervention::Continue, Some(InterventionOutcome::Unsupported))
                }
            }
            Intervention::ReplaceTool { original, replacement } => {
                if self.inject_prompt {
                    (
                        Intervention::InjectPrompt {
                            content: format!(
                                "Replace {} with {} (tool replacement unsupported, using prompt injection)",
                                original, replacement
                            ),
                            position: PromptPosition::BeforeNext,
                            prompt_id: None,
                        },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else if self.retry {
                    (
                        Intervention::Retry { delay_ms: 0 },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else if self.abort {
                    (
                        Intervention::Abort {
                            reason: "replace_tool unsupported, no fallback".into(),
                        },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else {
                    (Intervention::Continue, Some(InterventionOutcome::Unsupported))
                }
            }
            Intervention::Retry { .. } | Intervention::Backoff { .. } => {
                if self.abort {
                    (
                        Intervention::Abort {
                            reason: "retry unsupported".into(),
                        },
                        Some(InterventionOutcome::Unsupported),
                    )
                } else {
                    (Intervention::Continue, Some(InterventionOutcome::Unsupported))
                }
            }
            Intervention::Abort { .. } => {
                (Intervention::Continue, Some(InterventionOutcome::Unsupported))
            }
            Intervention::Custom { .. } => {
                (Intervention::Continue, Some(InterventionOutcome::Unsupported))
            }
            Intervention::Continue => (Intervention::Continue, None),
        };

        fallback
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Intervention {
    Continue,
    Retry {
        delay_ms: u64,
    },
    Backoff {
        base_ms: u64,
        attempt: u32,
    },
    ReplaceTool {
        original: String,
        replacement: String,
    },
    InjectPrompt {
        content: String,
        position: PromptPosition,

        #[serde(skip_serializing_if = "Option::is_none")]
        prompt_id: Option<String>,
    },
    Abort {
        reason: String,
    },
    #[serde(skip)]
    Custom {
        name: String,
        payload: Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PromptPosition {
    BeforeNext,
    ReplaceLast,
    AfterTool,
}

impl Intervention {
    pub fn is_continue(&self) -> bool {
        matches!(self, Intervention::Continue)
    }

    pub fn is_abort(&self) -> bool {
        matches!(self, Intervention::Abort { .. })
    }

    pub fn is_retry(&self) -> bool {
        matches!(self, Intervention::Retry { .. } | Intervention::Backoff { .. })
    }

    pub fn is_replace_tool(&self) -> bool {
        matches!(self, Intervention::ReplaceTool { .. })
    }

    pub fn is_inject_prompt(&self) -> bool {
        matches!(self, Intervention::InjectPrompt { .. })
    }

    pub fn delay_ms(&self) -> Option<u64> {
        match self {
            Intervention::Retry { delay_ms } => Some(*delay_ms),
            Intervention::Backoff { base_ms, attempt } => {
                Some(base_ms * 2u64.saturating_pow(*attempt))
            }
            _ => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Intervention::Abort { reason } => Some(reason),
            _ => None,
        }
    }

    pub fn prompt_content(&self) -> Option<&str> {
        match self {
            Intervention::InjectPrompt { content, .. } => Some(content),
            _ => None,
        }
    }
}

impl Default for Intervention {
    fn default() -> Self {
        Intervention::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continue() {
        let i = Intervention::Continue;
        assert!(i.is_continue());
        assert!(!i.is_abort());
        assert!(!i.is_retry());
    }

    #[test]
    fn test_abort() {
        let i = Intervention::Abort {
            reason: "too many retries".into(),
        };
        assert!(i.is_abort());
        assert_eq!(i.reason(), Some("too many retries"));
    }

    #[test]
    fn test_retry() {
        let i = Intervention::Retry { delay_ms: 100 };
        assert!(i.is_retry());
        assert_eq!(i.delay_ms(), Some(100));
    }

    #[test]
    fn test_backoff() {
        let i = Intervention::Backoff {
            base_ms: 100,
            attempt: 2,
        };
        assert!(i.is_retry());
        assert_eq!(i.delay_ms(), Some(400));
    }

    #[test]
    fn test_backoff_attempt_0() {
        let i = Intervention::Backoff {
            base_ms: 100,
            attempt: 0,
        };
        assert_eq!(i.delay_ms(), Some(100));
    }

    #[test]
    fn test_inject_prompt() {
        let i = Intervention::InjectPrompt {
            content: "try something else".into(),
            position: PromptPosition::BeforeNext,
            prompt_id: None,
        };
        assert!(i.is_inject_prompt());
        assert_eq!(i.prompt_content(), Some("try something else"));
    }

    #[test]
    fn test_replace_tool() {
        let i = Intervention::ReplaceTool {
            original: "delete".into(),
            replacement: "archive".into(),
        };
        assert!(i.is_replace_tool());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let i = Intervention::InjectPrompt {
            content: "hello".into(),
            position: PromptPosition::ReplaceLast,
            prompt_id: Some("test_prompt".into()),
        };
        let json = serde_json::to_string(&i).unwrap();
        let deserialized: Intervention = serde_json::from_str(&json).unwrap();
        assert_eq!(i, deserialized);
    }

    #[test]
    fn test_prompt_position_variants() {
        assert_ne!(
            serde_json::to_string(&PromptPosition::BeforeNext).unwrap(),
            serde_json::to_string(&PromptPosition::ReplaceLast).unwrap(),
        );
    }

    #[test]
    fn test_outcome_default() {
        let o = InterventionOutcome::Applied;
        assert!(matches!(o, InterventionOutcome::Applied));
    }

    #[test]
    fn test_capabilities_default() {
        let cap = AdapterCapabilities::default();
        assert!(cap.retry);
        assert!(!cap.inject_prompt);
        assert!(cap.abort);
        assert_eq!(cap.max_backoff_ms, 30_000);
    }

    #[test]
    fn test_downgrade_inject_prompt_no_capability() {
        let cap = AdapterCapabilities {
            retry: false,
            inject_prompt: false,
            abort: true,
            ..Default::default()
        };
        let intv = Intervention::InjectPrompt {
            content: "try different tool".into(),
            position: PromptPosition::BeforeNext,
            prompt_id: None,
        };
        let (downgraded, outcome) = cap.downgrade(&intv);
        assert!(downgraded.is_abort());
        assert_eq!(outcome, Some(InterventionOutcome::Unsupported));
    }

    #[test]
    fn test_downgrade_retry_falls_back_to_abort() {
        let cap = AdapterCapabilities {
            retry: false,
            inject_prompt: false,
            abort: true,
            ..Default::default()
        };
        let intv = Intervention::Retry { delay_ms: 100 };
        let (downgraded, outcome) = cap.downgrade(&intv);
        assert!(downgraded.is_abort());
        assert_eq!(outcome, Some(InterventionOutcome::Unsupported));
    }

    #[test]
    fn test_downgrade_replace_tool_falls_back_to_prompt() {
        let cap = AdapterCapabilities {
            retry: false,
            inject_prompt: true,
            replace_tool: false,
            ..Default::default()
        };
        let intv = Intervention::ReplaceTool {
            original: "delete".into(),
            replacement: "archive".into(),
        };
        let (downgraded, outcome) = cap.downgrade(&intv);
        assert!(downgraded.is_inject_prompt());
        assert_eq!(outcome, Some(InterventionOutcome::Unsupported));
    }

    #[test]
    fn test_downgrade_supported_stays_unchanged() {
        let cap = AdapterCapabilities {
            retry: true,
            inject_prompt: true,
            abort: true,
            ..Default::default()
        };
        let intv = Intervention::Retry { delay_ms: 100 };
        let (downgraded, outcome) = cap.downgrade(&intv);
        assert!(downgraded.is_retry());
        assert_eq!(outcome, None);
    }
}
