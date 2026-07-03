use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyConfig {

    pub port: u16,

    pub policy_path: PathBuf,

    pub openai_base_url: Option<String>,

    pub anthropic_base_url: Option<String>,

    pub persistent: bool,

    pub recover: bool,

    pub data_dir: PathBuf,

    pub log_dir: PathBuf,

    pub backups_dir: PathBuf,

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
            recover: false,
            log_dir: data_dir.join("logs"),
            backups_dir: data_dir.join("backups"),
            data_dir,
            sse_buffer_size: 5,
        }
    }
}

impl ProxyConfig {

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
