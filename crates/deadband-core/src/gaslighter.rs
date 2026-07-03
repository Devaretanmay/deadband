use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GaslighterTrial {
    /// Unique ID for this trial
    pub id: Uuid,
    /// The prompt_id of the injected prompt
    pub prompt_id: String,
    /// The kind of detection that triggered this intervention
    pub detection_kind: String,
    /// The execution session this trial belongs to
    pub execution_id: Uuid,
    /// When the prompt was injected
    pub injected_at: DateTime<Utc>,
    /// Whether the agent recovered (true = recovered, false = did not, None = pending)
    pub recovered: Option<bool>,
    /// How many steps it took to recover (None = pending / not applicable)
    pub steps_to_recover: Option<u32>,
    /// The step at which the prompt was injected
    pub injection_step: u64,
    /// The step at which recovery was assessed
    pub assessed_at_step: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptStats {
    /// The prompt ID
    pub prompt_id: String,
    /// Total number of trials with this prompt
    pub total_trials: u64,
    /// Number of successful recoveries
    pub successful_recoveries: u64,
    /// Recovery rate (0.0 - 1.0)
    pub recovery_rate: f64,
    /// Average steps to recover (when recovery occurred)
    pub avg_steps_to_recover: f64,
    /// Detection kinds this prompt was tested against
    pub detection_kinds: Vec<String>,
}

pub struct Gaslighter {
    /// All trials (completed + pending)
    trials: Vec<GaslighterTrial>,
    /// Recent steps by execution_id for recovery assessment
    recent_steps: HashMap<Uuid, Vec<(u64, bool)>>,
    /// How many steps to look ahead for recovery assessment
    recovery_window: u32,
    /// Maximum number of trials to keep in memory
    max_trials: usize,
}

impl Default for Gaslighter {
    fn default() -> Self {
        Self {
            trials: Vec::new(),
            recent_steps: HashMap::new(),
            recovery_window: 3,
            max_trials: 10_000,
        }
    }
}

impl Gaslighter {
    /// Create a new Gaslighter with the given recovery window.
    pub fn new(recovery_window: u32) -> Self {
        Self {
            recovery_window,
            ..Default::default()
        }
    }

    /// Record a prompt injection trial.
    pub fn record_injection(
        &mut self,
        prompt_id: String,
        detection_kind: String,
        execution_id: Uuid,
        injection_step: u64,
    ) {
        if self.trials.len() >= self.max_trials {
            self.trials.remove(0);
        }

        self.trials.push(GaslighterTrial {
            id: Uuid::new_v4(),
            prompt_id,
            detection_kind,
            execution_id,
            injected_at: Utc::now(),
            recovered: None,
            steps_to_recover: None,
            injection_step,
            assessed_at_step: None,
        });
    }

    /// Record a step in an execution. Used to assess recovery after prompt injection.
    /// Returns true if this step resolved any pending trials.
    pub fn record_step(&mut self, execution_id: Uuid, step: u64, is_success: bool) -> bool {
        let pending = self
            .trials
            .iter()
            .any(|t| t.execution_id == execution_id && t.recovered.is_none());

        if !pending {
            return false;
        }

        let entry = self
            .recent_steps
            .entry(execution_id)
            .or_insert_with(Vec::new);
        entry.push((step, is_success));

        // Trim old steps beyond the recovery window
        let cutoff = step.saturating_sub(self.recovery_window as u64);
        entry.retain(|(s, _)| *s >= cutoff);

        // Assess pending trials
        let mut resolved = false;
        for trial in &mut self.trials {
            if trial.execution_id != execution_id || trial.recovered.is_some() {
                continue;
            }

            let steps_since = step.saturating_sub(trial.injection_step);
            if steps_since > self.recovery_window as u64 {
                // Recovery window expired - mark as not recovered
                trial.recovered = Some(false);
                trial.steps_to_recover = None;
                trial.assessed_at_step = Some(step);
                resolved = true;
            } else if is_success {
                // Check if we have a success within the window
                let window_steps: Vec<_> = entry
                    .iter()
                    .filter(|(s, _)| *s > trial.injection_step)
                    .collect();
                if let Some(first_success) = window_steps.iter().find(|(_, ok)| *ok) {
                    trial.recovered = Some(true);
                    trial.steps_to_recover = Some(
                        first_success.0.saturating_sub(trial.injection_step) as u32,
                    );
                    trial.assessed_at_step = Some(step);
                    resolved = true;
                }
            }
        }

        resolved
    }

    /// Get statistics for a specific prompt.
    pub fn prompt_stats(&self, prompt_id: &str) -> Option<PromptStats> {
        let trials: Vec<_> = self
            .trials
            .iter()
            .filter(|t| t.prompt_id == prompt_id && t.recovered.is_some())
            .collect();

        if trials.is_empty() {
            return None;
        }

        let total = trials.len() as u64;
        let successful = trials.iter().filter(|t| t.recovered == Some(true)).count() as u64;
        let detection_kinds: Vec<String> = {
            let mut kinds: Vec<_> = trials
                .iter()
                .map(|t| t.detection_kind.clone())
                .collect();
            kinds.sort();
            kinds.dedup();
            kinds
        };

        let avg_steps: f64 = {
            let steps: Vec<u32> = trials
                .iter()
                .filter_map(|t| t.steps_to_recover)
                .collect();
            if steps.is_empty() {
                0.0
            } else {
                steps.iter().sum::<u32>() as f64 / steps.len() as f64
            }
        };

        Some(PromptStats {
            prompt_id: prompt_id.to_string(),
            total_trials: total,
            successful_recoveries: successful,
            recovery_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            avg_steps_to_recover: avg_steps,
            detection_kinds,
        })
    }

    /// Get stats for all prompts, ranked by recovery rate (descending).
    pub fn all_prompt_stats(&self) -> Vec<PromptStats> {
        let mut prompt_ids: Vec<String> = self
            .trials
            .iter()
            .map(|t| t.prompt_id.clone())
            .collect();
        prompt_ids.sort();
        prompt_ids.dedup();

        let mut stats: Vec<PromptStats> = prompt_ids
            .iter()
            .filter_map(|id| self.prompt_stats(id))
            .collect();

        stats.sort_by(|a, b| {
            b.recovery_rate
                .partial_cmp(&a.recovery_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        stats
    }

    /// Get all pending (unresolved) trials.
    pub fn pending_trials(&self) -> Vec<&GaslighterTrial> {
        self.trials
            .iter()
            .filter(|t| t.recovered.is_none())
            .collect()
    }

    /// Total number of trials recorded.
    pub fn total_trials(&self) -> usize {
        self.trials.len()
    }

    /// Number of completed (assessed) trials.
    pub fn completed_trials(&self) -> usize {
        self.trials.iter().filter(|t| t.recovered.is_some()).count()
    }

    /// Overall recovery rate across all prompts.
    pub fn overall_recovery_rate(&self) -> f64 {
        let completed = self.completed_trials();
        if completed == 0 {
            return 0.0;
        }
        let successful = self
            .trials
            .iter()
            .filter(|t| t.recovered == Some(true))
            .count();
        successful as f64 / completed as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_injection() {
        let mut g = Gaslighter::default();
        let exec_id = Uuid::new_v4();
        g.record_injection(
            "prompt_a".into(),
            "exact_repeat".into(),
            exec_id,
            0,
        );
        assert_eq!(g.total_trials(), 1);
        assert_eq!(g.pending_trials().len(), 1);
    }

    #[test]
    fn test_recovery_detected_within_window() {
        let mut g = Gaslighter::new(3);
        let exec_id = Uuid::new_v4();

        g.record_injection("prompt_a".into(), "exact_repeat".into(), exec_id, 1);
        assert_eq!(g.pending_trials().len(), 1);

        // Step 2: still failing
        g.record_step(exec_id, 2, false);
        assert_eq!(g.pending_trials().len(), 1);

        // Step 3: success! Should resolve the trial
        let resolved = g.record_step(exec_id, 3, true);
        assert!(resolved);
        assert_eq!(g.pending_trials().len(), 0);

        let stats = g.prompt_stats("prompt_a").unwrap();
        assert_eq!(stats.successful_recoveries, 1);
        assert_eq!(stats.recovery_rate, 1.0);
    }

    #[test]
    fn test_recovery_window_expires() {
        let mut g = Gaslighter::new(2);
        let exec_id = Uuid::new_v4();

        g.record_injection("prompt_b".into(), "error_pattern".into(), exec_id, 1);

        // Steps within window, all failures
        g.record_step(exec_id, 2, false);
        g.record_step(exec_id, 3, false);

        // Step 4: beyond the window, should expire the trial
        let resolved = g.record_step(exec_id, 4, false);
        assert!(resolved);
        assert_eq!(g.pending_trials().len(), 0);

        let stats = g.prompt_stats("prompt_b").unwrap();
        assert_eq!(stats.successful_recoveries, 0);
        assert_eq!(stats.recovery_rate, 0.0);
    }

    #[test]
    fn test_all_prompt_stats_ranking() {
        let mut g = Gaslighter::default();
        let exec_id = Uuid::new_v4();

        // Prompt A: 1/1 success
        g.record_injection("prompt_a".into(), "exact_repeat".into(), exec_id, 1);
        g.record_step(exec_id, 2, true);

        // Prompt B: 0/1 success
        g.record_injection("prompt_b".into(), "error_pattern".into(), exec_id, 3);
        g.record_step(exec_id, 10, false); // beyond window

        let stats = g.all_prompt_stats();
        assert_eq!(stats.len(), 2);
        // prompt_a (1.0) should be ranked above prompt_b (0.0)
        assert_eq!(stats[0].prompt_id, "prompt_a");
        assert_eq!(stats[1].prompt_id, "prompt_b");
    }

    #[test]
    fn test_overall_recovery_rate() {
        let mut g = Gaslighter::default();
        let exec_id = Uuid::new_v4();

        g.record_injection("prompt_a".into(), "exact_repeat".into(), exec_id, 1);
        g.record_step(exec_id, 2, true);

        g.record_injection("prompt_b".into(), "error_pattern".into(), exec_id, 3);
        g.record_step(exec_id, 10, false); // beyond window

        assert!((g.overall_recovery_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_multiple_injections_same_execution() {
        let mut g = Gaslighter::default();
        let exec_id = Uuid::new_v4();

        g.record_injection("prompt_a".into(), "exact_repeat".into(), exec_id, 1);
        g.record_injection("prompt_b".into(), "error_pattern".into(), exec_id, 3);

        // Step 5: success resolves prompt_b (within 3-step window)
        // and expires prompt_a (step 1 is 4 steps back, beyond 3-step window)
        g.record_step(exec_id, 5, true);

        // Both trials should be resolved: one succeeded, one expired
        assert_eq!(g.completed_trials(), 2);
        assert_eq!(g.pending_trials().len(), 0);

        let stats_a = g.prompt_stats("prompt_a").unwrap();
        assert_eq!(stats_a.successful_recoveries, 0); // expired

        let stats_b = g.prompt_stats("prompt_b").unwrap();
        assert_eq!(stats_b.successful_recoveries, 1); // recovered within window
    }
}
