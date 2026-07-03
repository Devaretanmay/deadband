use crate::detection::DetectorBox;
use crate::event::ToolCallEvent;
use crate::report::{DetectionReport, ObservationMetadata};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub semantic_enabled: bool,
    pub exact_enabled: bool,
    pub history_enabled: bool,
    pub rule_enabled: bool,
    pub semantic_model: Option<String>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            semantic_enabled: true,
            exact_enabled: true,
            history_enabled: true,
            rule_enabled: true,
            semantic_model: None,
        }
    }
}

pub struct ObservationPipeline {
    execution_id: Uuid,
    detectors: Vec<DetectorBox>,
    config: PipelineConfig,
}

impl ObservationPipeline {
    pub fn new(execution_id: Uuid, detectors: Vec<DetectorBox>, config: PipelineConfig) -> Self {
        Self {
            execution_id,
            detectors,
            config,
        }
    }

    pub fn observe(&self, event: ToolCallEvent, history: &[ToolCallEvent]) -> DetectionReport {
        let mut detections = Vec::new();
        for detector in &self.detectors {
            if let Some(detection) = detector.detect(&event, history) {
                detections.push(detection);
            }
        }

        let observation = ObservationMetadata {
            semantic_enabled: self.config.semantic_enabled,
            history_enabled: self.config.history_enabled,
            rule_enabled: self.config.rule_enabled,
            exact_enabled: self.config.exact_enabled,
            semantic_model: self.config.semantic_model.clone(),
        };

        DetectionReport::new(self.execution_id, event, detections, observation)
    }

    pub fn detector_count(&self) -> usize {
        self.detectors.len()
    }

    pub fn add_detector(&mut self, detector: DetectorBox) {
        self.detectors.push(detector);
    }
}
