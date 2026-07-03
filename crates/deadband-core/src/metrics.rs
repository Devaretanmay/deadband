
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::intervention::{Intervention, InterventionOutcome};

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        }
    }

    pub fn record_event(&mut self, event: MetricEvent) {
        match &event {
            MetricEvent::InterventionApplied { detection_kinds, .. } => {
                self.intervention_count += 1;
                for kind in detection_kinds {
                    *self.detection_breakdown.entry(kind.clone()).or_insert(0) += 1;
                }
            }
            MetricEvent::RecoverySuccessful { duration_ms, .. } => {
                self.recovery_time_ms = *duration_ms;
            }
            MetricEvent::ToolPrevented { .. } => {
                self.prevented_calls += 1;
            }
            MetricEvent::RetryPerformed { .. } => {}
        }
        self.events.push(event);
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
    use crate::intervention::InterventionOutcome;

    #[test]
    fn test_new_metrics_zero() {
        let m = RecoveryMetrics::new(Uuid::new_v4());
        assert_eq!(m.intervention_count, 0);
        assert_eq!(m.prevented_calls, 0);
        assert!(m.detection_breakdown.is_empty());
        assert!(m.events.is_empty());
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
