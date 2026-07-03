// Claude Code provider for Deadband

use crate::wrap::provider::AgentProvider;

/// Provider for Claude Code (Anthropic)
pub struct ClaudeProvider;

impl AgentProvider for ClaudeProvider {
    fn id(&self) -> &str { "claude" }
    
    fn name(&self) -> &str { "Claude Code" }
    
    fn build_env(&self, port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        vec![
            ("ANTHROPIC_BASE_URL", format!("http://127.0.0.1:{}", port)),
            ("ENABLE_TOOL_SEARCH", "true".to_string()),
        ]
    }
    
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        // Claude Code uses environment variables only, no config file modification needed
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn launch_command(&self, args: &[&str]) -> Vec<String> {
        let mut cmd = vec!["claude".to_string()];
        cmd.extend(args.iter().map(|s| s.to_string()));
        cmd
    }
    
    fn is_installed(&self) -> bool {
        std::process::Command::new("claude")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy is running. Configure Claude Code:");
        println!();
        println!("  Set environment variables:");
        println!("    export ANTHROPIC_BASE_URL=http://127.0.0.1:{}", port);
        println!("    export ENABLE_TOOL_SEARCH=true");
        println!();
        println!("  Then run Claude Code with your desired arguments.");
    }
}

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self
    }
}
