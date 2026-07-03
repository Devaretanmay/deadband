// Codex provider for Deadband

use crate::wrap::provider::AgentProvider;

/// Provider for OpenAI Codex CLI
/// Uses OpenAI API format
pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn id(&self) -> &str { "codex" }
    
    fn name(&self) -> &str { "Codex" }
    
    fn build_env(&self, port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        vec![
            ("OPENAI_BASE_URL", format!("http://127.0.0.1:{}/v1", port)),
        ]
    }
    
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        // Codex uses environment variables only
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn launch_command(&self, args: &[&str]) -> Vec<String> {
        let mut cmd = vec!["codex".to_string()];
        cmd.extend(args.iter().map(|s| s.to_string()));
        cmd
    }
    
    fn is_installed(&self) -> bool {
        std::process::Command::new("codex")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy is running. Configure Codex:");
        println!();
        println!("  Set environment variable:");
        println!("    export OPENAI_BASE_URL=http://127.0.0.1:{}/v1", port);
        println!();
        println!("  Then run Codex with your desired arguments.");
    }
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self
    }
}
