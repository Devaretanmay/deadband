// Intervention actions for Deadband
// Phase 1: Simple Block and InjectPrompt only

use serde::{Deserialize, Serialize};

/// Type of intervention to apply when a loop is detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Intervention {
    /// No intervention needed
    None,
    /// Block the request entirely
    Block {
        reason: String,
    },
    /// Inject a prompt into the response
    InjectPrompt {
        content: String,
    },
}

impl Default for Intervention {
    fn default() -> Self {
        Intervention::None
    }
}

impl Intervention {
    pub fn is_none(&self) -> bool {
        matches!(self, Intervention::None)
    }

    pub fn is_block(&self) -> bool {
        matches!(self, Intervention::Block { .. })
    }

    pub fn is_inject_prompt(&self) -> bool {
        matches!(self, Intervention::InjectPrompt { .. })
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Intervention::Block { reason } => Some(reason),
            _ => None,
        }
    }

    pub fn content(&self) -> Option<&str> {
        match self {
            Intervention::InjectPrompt { content } => Some(content),
            _ => None,
        }
    }
}

/// Create a block intervention
pub fn block(reason: impl Into<String>) -> Intervention {
    Intervention::Block {
        reason: reason.into(),
    }
}

/// Create an inject prompt intervention with default message
pub fn inject_prompt_default() -> Intervention {
    Intervention::InjectPrompt {
        content: "The previous tool call was part of a loop. Try a different approach.".to_string(),
    }
}

/// Create an inject prompt intervention with custom message
pub fn inject_prompt(content: impl Into<String>) -> Intervention {
    Intervention::InjectPrompt {
        content: content.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block() {
        let intervention = block("Loop detected");
        assert!(intervention.is_block());
        assert!(!intervention.is_none());
        assert_eq!(intervention.reason(), Some("Loop detected"));
    }

    #[test]
    fn test_inject_prompt_default() {
        let intervention = inject_prompt_default();
        assert!(intervention.is_inject_prompt());
        assert!(intervention.content().is_some());
    }

    #[test]
    fn test_inject_prompt_custom() {
        let intervention = inject_prompt("Custom message");
        assert!(intervention.is_inject_prompt());
        assert_eq!(intervention.content(), Some("Custom message"));
    }

    #[test]
    fn test_none() {
        let intervention = Intervention::None;
        assert!(intervention.is_none());
        assert!(!intervention.is_block());
        assert!(!intervention.is_inject_prompt());
    }
}
