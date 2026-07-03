use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Port to listen on (default 4399)
    pub port: u16,
    /// Path to policy YAML file
    pub policy_path: PathBuf,
    /// Upstream OpenAI-compatible base URL (override)
    pub openai_base_url: Option<String>,
    /// Upstream Anthropic-compatible base URL (override)
    pub anthropic_base_url: Option<String>,
    /// Whether to enable persistence (system service)
    pub persistent: bool,
    /// Data directory (~/.deadband)
    pub data_dir: PathBuf,
    /// Log directory
    pub log_dir: PathBuf,
    /// Backups directory
    pub backups_dir: PathBuf,
    /// SSE buffer size (number of chunks to buffer before detection)
    pub sse_buffer_size: usize,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".deadband");
        Self {
            port: 4399,
            policy_path: PathBuf::from("deadband.yaml"),
            openai_base_url: None,
            anthropic_base_url: None,
            persistent: false,
            log_dir: data_dir.join("logs"),
            backups_dir: data_dir.join("backups"),
            data_dir,
            sse_buffer_size: 5,
        }
    }
}

impl ProxyConfig {
    /// Data directory for storing logs, backups, etc.
    pub fn data_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".deadband")
    }

    pub fn log_dir() -> PathBuf {
        Self::data_dir().join("logs")
    }

    pub fn backups_dir() -> PathBuf {
        Self::data_dir().join("backups")
    }

    pub fn log_file() -> PathBuf {
        Self::log_dir().join("deadband.log")
    }
}
