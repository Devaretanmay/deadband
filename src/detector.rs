// Exact repeat detection for Deadband
// Phase 1: Simple in-memory detection of exact tool+args repeats

use serde_json::Value;
use std::collections::VecDeque;

/// Represents a tool call for detection purposes
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool_name: String,
    pub arguments: Value,
}

/// Simple loop detector using exact matching
/// Keeps a sliding window of recent calls and detects repeats
pub struct LoopDetector {
    history: VecDeque<ToolCall>,
    max_history: usize,
    threshold: u32,
}

impl LoopDetector {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
            threshold: 2, // Detect after 2 repeats
        }
    }

    /// Check if this tool call is a repeat
    /// Returns (is_loop, count) 
    pub fn check(&mut self, tool_name: String, arguments: Value) -> (bool, u32) {
        let call = ToolCall {
            tool_name: tool_name.clone(),
            arguments: arguments.clone(),
        };

        // Count how many times this exact call appears in history
        let count = self.history.iter()
            .filter(|c| c.tool_name == tool_name && c.arguments == arguments)
            .count() as u32;

        // Add to history
        self.history.push_back(call);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Total count includes the current call
        let total_count = count + 1;
        // A loop is detected when we've seen this call (threshold-1) times before
        // For threshold=2, we detect when count >= 1 (seen it once before)
        let is_loop = count >= self.threshold.saturating_sub(1);
        (is_loop, total_count)
    }

    /// Get current history size
    pub fn history_size(&self) -> usize {
        self.history.len()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_no_loop() {
        let mut detector = LoopDetector::new(10);
        
        // First call
        let (is_loop, count) = detector.check("search".to_string(), json!({"q": "hello"}));
        assert!(!is_loop);
        assert_eq!(count, 1);
        
        // Different call
        let (is_loop, count) = detector.check("search".to_string(), json!({"q": "world"}));
        assert!(!is_loop);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_loop_detected() {
        let mut detector = LoopDetector::new(10);
        
        let args = json!({"q": "hello"});
        
        // First call
        let (is_loop, count) = detector.check("search".to_string(), args.clone());
        assert!(!is_loop);
        assert_eq!(count, 1);
        
        // Second call (same) - triggers loop
        let (is_loop, count) = detector.check("search".to_string(), args.clone());
        assert!(is_loop);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_different_tools_no_loop() {
        let mut detector = LoopDetector::new(10);
        
        let args = json!({"q": "hello"});
        
        // First tool
        let (is_loop, _) = detector.check("search".to_string(), args.clone());
        assert!(!is_loop);
        
        // Same args, different tool
        let (is_loop, _) = detector.check("fetch".to_string(), args);
        assert!(!is_loop);
    }

    #[test]
    fn test_history_bounded() {
        let mut detector = LoopDetector::new(3);
        
        for i in 0..5 {
            detector.check("tool".to_string(), json!({"id": i}));
        }
        
        assert_eq!(detector.history_size(), 3);
    }
}
