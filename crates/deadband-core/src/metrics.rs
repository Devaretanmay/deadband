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
        report: &deadband_observation::report::DetectionReport,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VitalSigns {
    /// Total number of loops detected
    pub loop_count: u64,
    /// Average detection time in milliseconds
    pub avg_detection_time_ms: f64,
    /// Success rate (recoveries / interventions), 0.0 - 1.0
    pub success_rate: f64,
    /// Estimated API spend in USD
    pub api_spend: f64,
    /// Total events processed
    pub total_events: u64,
    /// Total interventions applied
    pub total_interventions: u64,
    /// Breakdown of intervention types
    pub intervention_breakdown: std::collections::HashMap<String, u64>,
    /// Breakdown of detection kinds
    pub detection_breakdown: std::collections::HashMap<String, u64>,
    /// Total detection time in milliseconds (for computing average)
    pub total_detection_time_ms: u64,
}

impl VitalSigns {
    /// Compute vital signs from RecoveryMetrics.
    pub fn from_recovery_metrics(metrics: &RecoveryMetrics) -> Self {
        let total_interventions = metrics.intervention_count;
        let loop_count = metrics.total_detections();
        let detection_breakdown = metrics.detection_breakdown.clone();

        // Compute success rate from recovery events
        let mut success_count = 0u64;
        let mut intervention_breakdown = std::collections::HashMap::new();
        for event in &metrics.events {
            match event {
                MetricEvent::RecoverySuccessful { .. } => {
                    success_count += 1;
                }
                MetricEvent::InterventionApplied { outcome, .. } => {
                    let key = format!("{:?}", outcome);
                    *intervention_breakdown.entry(key).or_insert(0) += 1;
                }
                _ => {}
            }
        }

        let success_rate = if total_interventions > 0 && loop_count > 0 {
            success_count as f64 / loop_count.min(total_interventions) as f64
        } else {
            1.0 // Default to 100% if no data
        };

        // Estimate API spend (rough: ~$0.01 per event for typical LLM calls)
        let api_spend = (metrics.execution_id.to_string().len() as f64 * 0.0001) // nominal
            + (loop_count as f64 * 0.001); // ~$0.001 per detection for analysis

        Self {
            loop_count,
            avg_detection_time_ms: 0.0, // Filled externally if timing data available
            success_rate: success_rate.min(1.0),
            api_spend,
            total_events: metrics.events.len() as u64,
            total_interventions,
            intervention_breakdown,
            detection_breakdown,
            total_detection_time_ms: metrics.recovery_time_ms,
        }
    }

    /// Estimate total cost based on event count and typical token usage.
    /// Useful for tracking experiment spending.
    pub fn estimated_cost(&self, cost_per_1k_tokens: f64) -> f64 {
        self.total_events as f64 * cost_per_1k_tokens * 0.5 // assume ~500 tokens per event
    }

    /// Combine multiple VitalSigns snapshots (e.g., from different sessions).
    pub fn merge(snapshots: &[VitalSigns]) -> Self {
        let mut merged = VitalSigns::default();
        for vs in snapshots {
            merged.loop_count += vs.loop_count;
            merged.avg_detection_time_ms += vs.avg_detection_time_ms;
            merged.api_spend += vs.api_spend;
            merged.total_events += vs.total_events;
            merged.total_interventions += vs.total_interventions;
            merged.total_detection_time_ms += vs.total_detection_time_ms;
            for (k, v) in &vs.detection_breakdown {
                *merged.detection_breakdown.entry(k.clone()).or_insert(0) += v;
            }
            for (k, v) in &vs.intervention_breakdown {
                *merged.intervention_breakdown.entry(k.clone()).or_insert(0) += v;
            }
        }
        if !snapshots.is_empty() {
            let count = snapshots.len() as f64;
            merged.avg_detection_time_ms /= count;
            let total_attempts = merged.loop_count.max(1);
            merged.success_rate = (merged.loop_count as f64 / total_attempts as f64).min(1.0);
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use deadband_observation::event::ErrorKind;
    use deadband_observation::event::ToolCallEvent;
    use crate::intervention::InterventionOutcome;
    use deadband_observation::report::{DetectionReport, ObservationMetadata};
    use serde_json::json;

    fn make_report(kind: &str) -> DetectionReport {
        let detection = match kind {
            "exact_repeat" => deadband_observation::detection::Detection::ExactRepeat {
                tool: "x".into(),
                count: 3,
            },
            "error_pattern" => deadband_observation::detection::Detection::ErrorPattern {
                kind: ErrorKind::Timeout,
                count: 3,
            },
            _ => deadband_observation::detection::Detection::RuleViolation {
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
