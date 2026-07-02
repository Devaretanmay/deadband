use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::detection::Detection;
use crate::event::ToolCallEvent;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionSummary {
    pub highest_severity: Severity,
    pub confidence: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub execution_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationMetadata {
    pub semantic_enabled: bool,
    pub budget_enabled: bool,
    pub history_enabled: bool,
    pub rule_enabled: bool,
    pub exact_enabled: bool,
    pub semantic_model: Option<String>,
}

impl Default for ObservationMetadata {
    fn default() -> Self {
        Self {
            semantic_enabled: false,
            budget_enabled: false,
            history_enabled: false,
            rule_enabled: false,
            exact_enabled: false,
            semantic_model: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionReport {
    pub event: ToolCallEvent,
    pub detections: Vec<Detection>,
    pub summary: DetectionSummary,
    pub metadata: ReportMetadata,
    pub observation: ObservationMetadata,
}

impl DetectionReport {
    pub fn new(
        execution_id: Uuid,
        event: ToolCallEvent,
        detections: Vec<Detection>,
        observation: ObservationMetadata,
    ) -> Self {
        let mut highest_severity = Severity::Low;
        let mut confidence = 0.0;

        for d in &detections {
            let (sev, conf) = d.intrinsic_severity_and_confidence();
            if sev > highest_severity {
                highest_severity = sev;
            }
            if conf > confidence {
                confidence = conf;
            }
        }

        Self {
            event,
            detections,
            summary: DetectionSummary {
                highest_severity,
                confidence,
            },
            metadata: ReportMetadata {
                execution_id,
                timestamp: Utc::now(),
            },
            observation,
        }
    }
}
