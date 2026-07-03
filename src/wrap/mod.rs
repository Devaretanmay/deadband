// Deadband wrap module
// Provides `deadband wrap <agent>` command to configure agents through the proxy

pub mod provider;
pub mod utils;

pub mod agents {
    pub mod claude;
    pub mod aider;
    pub mod codex;
    pub mod opencode;
    pub mod cursor;
    pub mod continue_dev;
    pub mod cline;
    pub mod vibe;
}

// Re-export for easier access
pub use provider::AgentProvider;
