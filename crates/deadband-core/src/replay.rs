use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use deadband_observation::event::ToolCallEvent;
use deadband_observation::report::DetectionReport;
use crate::intervention::Intervention;
use crate::metrics::{MetricEvent, RecoveryMetrics};

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Trace validation error: {0}")]
    ValidationError(String),
    #[error("Unsupported trace version: {0}")]
    UnsupportedVersion(u8),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub version: u8,
    pub execution_id: Uuid,
    pub schema_version: u32,
    pub started_at: DateTime<Utc>,
    pub events: Vec<ToolCallEvent>,
    pub interventions: Vec<InterventionRecord>,
    pub metrics: RecoveryMetrics,
    pub policy_config: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterventionRecord {
    pub event_index: usize,
    pub event: ToolCallEvent,
    pub report: Option<DetectionReport>,
    pub intervention: Intervention,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayResult {
    pub total_events: usize,
    pub total_interventions: usize,
    pub prevented_calls: u64,
    pub recovery_time_ms: u64,
    pub matched: bool,
    pub divergences: Vec<EventDiff>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventDiff {
    pub index: usize,
    pub differences: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceSnapshot {
    pub event_index: usize,
    pub event: ToolCallEvent,
    pub report: Option<DetectionReport>,
    pub intervention: Option<Intervention>,
}

pub struct Replayer;

impl Replayer {
    pub const VERSION: u8 = 1;
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn from_json(path: impl AsRef<Path>) -> Result<Trace, ReplayError> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let raw: serde_json::Value = serde_json::from_str(&content)?;
        let version = raw.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        if version != Self::VERSION {
            return Err(ReplayError::UnsupportedVersion(version));
        }
        let trace: Trace = serde_json::from_str(&content)?;
        Self::validate(&trace)?;
        Ok(trace)
    }

    pub fn to_json(trace: &Trace, path: impl AsRef<Path>) -> Result<(), ReplayError> {
        let content = serde_json::to_string_pretty(trace)?;
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }

    pub fn trace_to_string(trace: &Trace) -> Result<String, ReplayError> {
        Ok(serde_json::to_string_pretty(trace)?)
    }

    pub fn validate(trace: &Trace) -> Result<(), ReplayError> {
        if trace.version != Self::VERSION {
            return Err(ReplayError::UnsupportedVersion(trace.version));
        }
        if trace.events.is_empty() {
            return Err(ReplayError::ValidationError(
                "Trace has no events".into(),
            ));
        }
        for record in &trace.interventions {
            if record.event_index >= trace.events.len() {
                return Err(ReplayError::ValidationError(format!(
                    "Intervention references event index {} but only {} events exist",
                    record.event_index,
                    trace.events.len()
                )));
            }
        }
        Ok(())
    }

    pub fn compare(original: &Trace, replayed: &Trace) -> ReplayResult {
        if original.version != replayed.version {
            return ReplayResult {
                total_events: replayed.events.len(),
                total_interventions: replayed.interventions.len(),
                prevented_calls: 0,
                recovery_time_ms: 0,
                matched: false,
                divergences: vec![EventDiff {
                    index: 0,
                    differences: vec![format!(
                        "version mismatch: {} vs {}",
                        original.version, replayed.version
                    )],
                }],
            };
        }

        let max_len = original.events.len().max(replayed.events.len());
        let mut divergences = Vec::new();

        for i in 0..max_len {
            let mut diffs = Vec::new();
            let orig_event = original.events.get(i);
            let replay_event = replayed.events.get(i);

            match (orig_event, replay_event) {
                (Some(a), Some(b)) => {
                    if a.tool_name != b.tool_name {
                        diffs.push(format!(
                            "tool_name mismatch: {} vs {}",
                            a.tool_name, b.tool_name
                        ));
                    }
                    if a.arguments != b.arguments {
                        diffs.push("arguments differ".into());
                    }
                }
                (None, Some(_)) => {
                    diffs.push("unexpected event in replayed trace".into());
                }
                (Some(_), None) => {
                    diffs.push("missing event in replayed trace".into());
                }
                (None, None) => {}
            }

            let orig_int = original.interventions.get(i);
            let replay_int = replayed.interventions.get(i);

            match (orig_int, replay_int) {
                (Some(a), Some(b)) => {
                    if a.intervention != b.intervention {
                        diffs.push(format!(
                            "intervention mismatch: {:?} vs {:?}",
                            a.intervention, b.intervention
                        ));
                    }
                }
                (Some(_), None) => {
                    divergences.push(EventDiff {
                        index: i,
                        differences: vec!["expected intervention not generated".into()],
                    });
                    continue;
                }
                (None, Some(_)) => {
                    divergences.push(EventDiff {
                        index: i,
                        differences: vec!["unexpected intervention generated".into()],
                    });
                    continue;
                }
                (None, None) => {}
            }

            if !diffs.is_empty() {
                divergences.push(EventDiff {
                    index: i,
                    differences: diffs,
                });
            }
        }

        ReplayResult {
            total_events: replayed.events.len(),
            total_interventions: replayed.interventions.len(),
            prevented_calls: replayed
                .metrics
                .events
                .iter()
                .filter(|e| matches!(e, MetricEvent::ToolPrevented { .. }))
                .count() as u64,
            recovery_time_ms: replayed.metrics.recovery_time_ms,
            matched: divergences.is_empty(),
            divergences,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_trace(prefix: &str, count: usize) -> Trace {
        let execution_id = Uuid::new_v4();
        let events: Vec<ToolCallEvent> = (0..count)
            .map(|i| {
                ToolCallEvent::started(
                    "test",
                    i as u64,
                    &format!("{}_{}", prefix, i),
                    json!({"idx": i}),
                )
            })
            .collect();
        Trace {
            version: Replayer::VERSION,
            execution_id,
            schema_version: Replayer::SCHEMA_VERSION,
            started_at: Utc::now(),
            events,
            interventions: Vec::new(),
            metrics: RecoveryMetrics::new(execution_id),
            policy_config: String::new(),
        }
    }

    #[test]
    fn test_validate_empty_trace_fails() {
        let trace = Trace {
            version: Replayer::VERSION,
            execution_id: Uuid::new_v4(),
            schema_version: Replayer::SCHEMA_VERSION,
            started_at: Utc::now(),
            events: vec![],
            interventions: vec![],
            metrics: RecoveryMetrics::new(Uuid::new_v4()),
            policy_config: String::new(),
        };
        assert!(Replayer::validate(&trace).is_err());
    }

    #[test]
    fn test_validate_valid_trace_ok() {
        let trace = make_trace("event", 3);
        assert!(Replayer::validate(&trace).is_ok());
    }

    #[test]
    fn test_validate_bad_intervention_index() {
        let mut trace = make_trace("e", 3);
        trace.interventions.push(InterventionRecord {
            event_index: 10,
            event: trace.events[0].clone(),
            report: None,
            intervention: Intervention::Continue,
        });
        assert!(Replayer::validate(&trace).is_err());
    }

    #[test]
    fn test_compare_identical_traces() {
        let a = make_trace("x", 5);
        let b = make_trace("x", 5);
        let result = Replayer::compare(&a, &b);
        assert!(result.matched);
        assert!(result.divergences.is_empty());
    }

    #[test]
    fn test_compare_different_lengths() {
        let a = make_trace("x", 5);
        let b = make_trace("x", 3);
        let result = Replayer::compare(&a, &b);
        assert!(!result.matched);
        assert!(!result.divergences.is_empty());
    }

    #[test]
    fn test_roundtrip_json() {
        let trace = make_trace("test", 3);
        let json = serde_json::to_string_pretty(&trace).unwrap();
        let restored: Trace = serde_json::from_str(&json).unwrap();
        assert_eq!(trace.execution_id, restored.execution_id);
        assert_eq!(trace.events.len(), restored.events.len());
        assert_eq!(trace.version, restored.version);
    }

    #[test]
    fn test_unsupported_version() {
        let mut trace = make_trace("t", 1);
        trace.version = 99;
        let json = serde_json::to_string(&trace).unwrap();
        let path = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(&path, json).unwrap();
        let err = Replayer::from_json(&path).unwrap_err();
        assert!(matches!(err, ReplayError::UnsupportedVersion(99)));
    }
}
