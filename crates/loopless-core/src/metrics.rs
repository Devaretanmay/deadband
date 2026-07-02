use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::intervention::{Intervention, InterventionOutcome};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MetricEvent {
    InterventionApplied {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
        detection_kinds: Vec<String>,
        outcome: InterventionOutcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        downgraded_from: Option<Intervention>,
    },
    ToolPrevented {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
        reason: String,
    },
    RetryPerformed {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
        delay_ms: u64,
        attempt: u32,
    },
    RecoverySuccessful {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
        duration_ms: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub intervention_count: u64,
    pub recovery_time_ms: u64,
    pub prevented_calls: u64,
    pub detection_breakdown: HashMap<String, u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryMetrics {
    pub execution_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub runtime_ms: u64,
    pub intervention_count: u64,
    pub recovery_time_ms: u64,
    pub prevented_calls: u64,
    pub loop_duration_ms: u64,
    pub detection_breakdown: HashMap<String, u64>,
    pub events: Vec<MetricEvent>,
    snapshot: MetricsSnapshot,
}

impl RecoveryMetrics {
    pub fn new(execution_id: Uuid) -> Self {
        Self {
            execution_id,
            started_at: Utc::now(),
            runtime_ms: 0,
            intervention_count: 0,
            recovery_time_ms: 0,
            prevented_calls: 0,
            loop_duration_ms: 0,
            detection_breakdown: HashMap::new(),
            events: Vec::new(),
            snapshot: MetricsSnapshot {
                intervention_count: 0,
                recovery_time_ms: 0,
                prevented_calls: 0,
                detection_breakdown: HashMap::new(),
            },
        }
    }

    pub fn record_event(&mut self, event: MetricEvent) {
        self.sync_legacy_counters(&event);
        self.events.push(event);
    }

    pub fn project_metrics(&self) -> MetricsSnapshot {
        let mut snapshot = MetricsSnapshot {
            intervention_count: 0,
            recovery_time_ms: self.recovery_time_ms,
            prevented_calls: 0,
            detection_breakdown: HashMap::new(),
        };

        for event in &self.events {
            match event {
                MetricEvent::InterventionApplied { detection_kinds, .. } => {
                    snapshot.intervention_count += 1;
                    for kind in detection_kinds {
                        *snapshot
                            .detection_breakdown
                            .entry(kind.clone())
                            .or_insert(0) += 1;
                    }
                }
                MetricEvent::RetryPerformed { .. } => {}
                MetricEvent::RecoverySuccessful { duration_ms, .. } => {
                    snapshot.recovery_time_ms = *duration_ms;
                }
                MetricEvent::ToolPrevented { .. } => {}
            }
        }
        snapshot
    }

    pub fn snapshot(&self) -> &MetricsSnapshot {
        &self.snapshot
    }

    fn sync_legacy_counters(&mut self, event: &MetricEvent) {
        match event {
            MetricEvent::InterventionApplied { detection_kinds, .. } => {
                self.intervention_count += 1;
                for kind in detection_kinds {
                    *self
                        .detection_breakdown
                        .entry(kind.clone())
                        .or_insert(0) += 1;
                }
            }
            MetricEvent::RecoverySuccessful { duration_ms, .. } => {
                self.recovery_time_ms = *duration_ms;
            }
            MetricEvent::RetryPerformed { .. } => {}
            MetricEvent::ToolPrevented { .. } => {
                self.prevented_calls += 1;
            }
        }
        self.snapshot = MetricsSnapshot {
            intervention_count: self.intervention_count,
            recovery_time_ms: self.recovery_time_ms,
            prevented_calls: self.prevented_calls,
            detection_breakdown: self.detection_breakdown.clone(),
        };
    }

    // DEPRECATED: Use `record_event` instead.
    pub fn record_intervention(
        &mut self,
        report: &loopless_observation::report::DetectionReport,
        _duration_ms: u64,
    ) {
        self.intervention_count += 1;
        for detection in &report.detections {
            *self
                .detection_breakdown
                .entry(detection.kind().to_string())
                .or_insert(0) += 1;
        }
        self.snapshot = MetricsSnapshot {
            intervention_count: self.intervention_count,
            recovery_time_ms: self.recovery_time_ms,
            prevented_calls: self.prevented_calls,
            detection_breakdown: self.detection_breakdown.clone(),
        };
    }

    // DEPRECATED: Use `record_event` instead.
    pub fn record_prevented_call(&mut self) {
        self.prevented_calls += 1;
        self.snapshot = MetricsSnapshot {
            intervention_count: self.intervention_count,
            recovery_time_ms: self.recovery_time_ms,
            prevented_calls: self.prevented_calls,
            detection_breakdown: self.detection_breakdown.clone(),
        };
    }

    // DEPRECATED: Use `record_event` instead.
    pub fn record_recovery_time(&mut self, ms: u64) {
        self.recovery_time_ms = ms;
        self.snapshot = MetricsSnapshot {
            intervention_count: self.intervention_count,
            recovery_time_ms: self.recovery_time_ms,
            prevented_calls: self.prevented_calls,
            detection_breakdown: self.detection_breakdown.clone(),
        };
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn total_detections(&self) -> u64 {
        self.detection_breakdown.values().sum()
    }
}

impl Default for RecoveryMetrics {
    fn default() -> Self {
        Self::new(Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use loopless_observation::event::ErrorKind;
    use loopless_observation::event::ToolCallEvent;
    use crate::intervention::InterventionOutcome;
    use loopless_observation::report::{DetectionReport, ObservationMetadata};
    use serde_json::json;

    fn make_report(kind: &str) -> DetectionReport {
        let detection = match kind {
            "exact_repeat" => loopless_observation::detection::Detection::ExactRepeat {
                tool: "x".into(),
                count: 3,
            },
            "error_pattern" => loopless_observation::detection::Detection::ErrorPattern {
                kind: ErrorKind::Timeout,
                count: 3,
            },
            _ => loopless_observation::detection::Detection::RuleViolation {
                rule: "r".into(),
                detail: "d".into(),
            },
        };
        let event = ToolCallEvent::started("test", 0, "x", json!({}));
        DetectionReport::new(
            Uuid::new_v4(),
            event,
            vec![detection],
            ObservationMetadata::default(),
        )
    }

    #[test]
    fn test_new_metrics_zero() {
        let m = RecoveryMetrics::new(Uuid::new_v4());
        assert_eq!(m.intervention_count, 0);
        assert_eq!(m.prevented_calls, 0);
        assert!(m.detection_breakdown.is_empty());
        assert!(m.events.is_empty());
    }

    #[test]
    fn test_record_intervention() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_intervention(&make_report("exact_repeat"), 0);
        assert_eq!(m.intervention_count, 1);
        assert_eq!(*m.detection_breakdown.get("exact_repeat").unwrap(), 1);
    }

    #[test]
    fn test_record_event_intervention() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_event(MetricEvent::InterventionApplied {
            execution_id: m.execution_id,
            timestamp: Utc::now(),
            detection_kinds: vec!["exact_repeat".into()],
            outcome: InterventionOutcome::Applied,
            downgraded_from: None,
        });
        assert_eq!(m.intervention_count, 1);
        assert_eq!(m.snapshot().intervention_count, 1);
    }

    #[test]
    fn test_project_metrics() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_event(MetricEvent::InterventionApplied {
            execution_id: m.execution_id,
            timestamp: Utc::now(),
            detection_kinds: vec!["exact_repeat".into(), "error_pattern".into()],
            outcome: InterventionOutcome::Applied,
            downgraded_from: None,
        });
        let snapshot = m.project_metrics();
        assert_eq!(snapshot.intervention_count, 1);
        assert_eq!(snapshot.detection_breakdown.get("exact_repeat"), Some(&1));
        assert_eq!(snapshot.detection_breakdown.get("error_pattern"), Some(&1));
    }

    #[test]
    fn test_record_multiple_types() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_intervention(&make_report("exact_repeat"), 0);
        m.record_intervention(&make_report("error_pattern"), 0);
        assert_eq!(m.intervention_count, 2);
        assert_eq!(m.total_detections(), 2);
    }

    #[test]
    fn test_record_prevented_calls() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_prevented_call();
        m.record_prevented_call();
        assert_eq!(m.prevented_calls, 2);
    }

    #[test]
    fn test_record_retry_event() {
        let mut m = RecoveryMetrics::new(Uuid::new_v4());
        m.record_event(MetricEvent::RetryPerformed {
            execution_id: m.execution_id,
            timestamp: Utc::now(),
            delay_ms: 100,
            attempt: 1,
        });
        assert_eq!(m.intervention_count, 0);
        assert_eq!(m.prevented_calls, 0);
    }

    #[test]
    fn test_to_json() {
        let m = RecoveryMetrics::new(Uuid::new_v4());
        let json = m.to_json();
        assert!(json.contains("execution_id"));
    }
}
