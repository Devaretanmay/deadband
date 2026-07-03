use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ToolCallEvent {
    pub version: u8,
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub thread_id: String,
    pub step: u64,
    pub tool_name: String,
    pub arguments: Value,
    #[serde(flatten)]
    pub payload: Payload,
}

impl ToolCallEvent {
    pub fn new(
        thread_id: impl Into<String>,
        step: u64,
        tool_name: impl Into<String>,
        arguments: Value,
        payload: Payload,
    ) -> Self {
        Self {
            version: 1,
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            thread_id: thread_id.into(),
            step,
            tool_name: tool_name.into(),
            arguments,
            payload,
        }
    }

    pub fn started(
        thread_id: impl Into<String>,
        step: u64,
        tool_name: impl Into<String>,
        arguments: Value,
    ) -> Self {
        Self::new(
            thread_id,
            step,
            tool_name,
            arguments.clone(),
            Payload::Started { input: arguments },
        )
    }

    pub fn succeeded(
        thread_id: impl Into<String>,
        step: u64,
        tool_name: impl Into<String>,
        arguments: Value,
        output: Value,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            thread_id,
            step,
            tool_name,
            arguments,
            Payload::Succeeded { output, duration_ms },
        )
    }

    pub fn failed(
        thread_id: impl Into<String>,
        step: u64,
        tool_name: impl Into<String>,
        arguments: Value,
        error: ErrorKind,
        message: String,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            thread_id,
            step,
            tool_name,
            arguments,
            Payload::Failed {
                error,
                message,
                duration_ms,
            },
        )
    }

    pub fn is_started(&self) -> bool {
        matches!(self.payload, Payload::Started { .. })
    }

    pub fn is_succeeded(&self) -> bool {
        matches!(self.payload, Payload::Succeeded { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.payload, Payload::Failed { .. })
    }

    pub fn error_kind(&self) -> Option<ErrorKind> {
        match &self.payload {
            Payload::Failed { error, .. } => Some(*error),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "Unsupported ToolCallEvent version: {}",
                self.version
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status")]
pub enum Payload {
    #[serde(rename = "started")]
    Started {
        input: Value,
    },
    #[serde(rename = "succeeded")]
    Succeeded {
        output: Value,
        duration_ms: u64,
    },
    #[serde(rename = "failed")]
    Failed {
        error: ErrorKind,
        message: String,
        duration_ms: u64,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Timeout,
    Validation,
    Permission,
    NotFound,
    Network,
    RateLimit,
    Internal,
    Semantic,
    Unknown,
}

impl ErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorKind::Timeout => "timeout",
            ErrorKind::Validation => "validation",
            ErrorKind::Permission => "permission",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Network => "network",
            ErrorKind::RateLimit => "rate_limit",
            ErrorKind::Internal => "internal",
            ErrorKind::Semantic => "semantic",
            ErrorKind::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_started_event() {
        let event = ToolCallEvent::started(
            "thread-1",
            1,
            "search",
            json!({"q": "hello"}),
        );
        assert_eq!(event.version, 1);
        assert!(event.is_started());
        assert_eq!(event.tool_name, "search");
    }

    #[test]
    fn test_succeeded_event() {
        let event = ToolCallEvent::succeeded(
            "thread-1",
            1,
            "search",
            json!({"q": "hello"}),
            json!({"result": "world"}),
            42,
        );
        assert!(event.is_succeeded());
        assert_eq!(event.step, 1);
    }

    #[test]
    fn test_failed_event() {
        let event = ToolCallEvent::failed(
            "thread-1",
            1,
            "search",
            json!({"q": "hello"}),
            ErrorKind::Timeout,
            "connection timed out".into(),
            5000,
        );
        assert!(event.is_failed());
        assert_eq!(event.error_kind(), Some(ErrorKind::Timeout));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let event = ToolCallEvent::started(
            "thread-1",
            1,
            "search",
            json!({"q": "hello"}),
        );
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ToolCallEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.tool_name, deserialized.tool_name);
        assert!(deserialized.is_started());
    }

    #[test]
    fn test_error_kind_as_str() {
        assert_eq!(ErrorKind::Timeout.as_str(), "timeout");
        assert_eq!(ErrorKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_unique_ids() {
        let a = ToolCallEvent::started("t", 0, "a", json!({}));
        let b = ToolCallEvent::started("t", 0, "a", json!({}));
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_validate_version_ok() {
        let event = ToolCallEvent::started("t", 0, "a", json!({}));
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_validate_version_bad() {
        let mut event = ToolCallEvent::started("t", 0, "a", json!({}));
        event.version = 99;
        assert!(event.validate().is_err());
    }
}
