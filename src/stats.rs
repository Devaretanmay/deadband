// Simple statistics tracking for Deadband

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_requests: u64,
    pub loops_detected: u64,
    pub interventions_applied: u64,
    pub calls_prevented: u64,
    pub status: String,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            loops_detected: 0,
            interventions_applied: 0,
            calls_prevented: 0,
            status: "stopped".to_string(),
        }
    }
}

impl Stats {
    pub fn estimated_savings(&self) -> f64 {
        // Assume $0.002 per prevented call
        self.calls_prevented as f64 * 0.002
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), anyhow::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let json = std::fs::read_to_string(path)?;
        let stats: Stats = serde_json::from_str(&json)?;
        Ok(stats)
    }

    pub fn record_request(&mut self) {
        self.total_requests += 1;
    }

    pub fn record_loop(&mut self) {
        self.loops_detected += 1;
        self.calls_prevented += 1;
    }

    pub fn record_intervention(&mut self) {
        self.interventions_applied += 1;
    }
}
