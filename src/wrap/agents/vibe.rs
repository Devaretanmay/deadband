// Mistral Vibe provider for Deadband

use crate::wrap::provider::AgentProvider;

/// Provider for Mistral Vibe CLI
/// Supports both OpenAI and Anthropic APIs
pub struct VibeProvider;

impl AgentProvider for VibeProvider {
    fn id(&self) -> &str { "vibe" }
    
    fn name(&self) -> &str { "Mistral Vibe" }
    
    fn build_env(&self, port: u16, project: Option<&str>) -> Vec<(&str, String)> {
        let project_prefix = project.map(|p| format!("/p/{}", p)).unwrap_or_default();
        let openai_base = format!("http://127.0.0.1:{}/v1{}", port, project_prefix);
        let anthropic_base = format!("http://127.0.0.1:{}", port);
        
        vec![
            ("OPENAI_BASE_URL", openai_base),
            ("ANTHROPIC_BASE_URL", anthropic_base),
        ]
    }
    
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        // Vibe uses environment variables only
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn launch_command(&self, args: &[&str]) -> Vec<String> {
        let mut cmd = vec!["vibe".to_string()];
        cmd.extend(args.iter().map(|s| s.to_string()));
        cmd
    }
    
    fn is_installed(&self) -> bool {
        std::process::Command::new("vibe")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        let project = crate::wrap::utils::get_project_name().unwrap_or_default();
        let project_prefix = if project.is_empty() { "".to_string() } else { format!("/p/{}", project) };
        let openai_base = format!("http://127.0.0.1:{}/v1{}", port, project_prefix);
        let anthropic_base = format!("http://127.0.0.1:{}", port);
        
        println!("  Deadband proxy is running. Configure Mistral Vibe:");
        println!();
        println!("  Set environment variables:");
        println!("    export OPENAI_BASE_URL={}", openai_base);
        println!("    export ANTHROPIC_BASE_URL={}", anthropic_base);
        println!();
        println!("  Then run Mistral Vibe with your desired arguments.");
    }
}

impl Default for VibeProvider {
    fn default() -> Self {
        Self
    }
}
