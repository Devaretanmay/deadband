// Cursor provider for Deadband
// Cursor is a VS Code extension that requires manual setup

use crate::wrap::provider::AgentProvider;

/// Provider for Cursor (VS Code extension)
/// Requires manual setup in VS Code settings
pub struct CursorProvider;

impl AgentProvider for CursorProvider {
    fn id(&self) -> &str { "cursor" }
    
    fn name(&self) -> &str { "Cursor" }
    
    fn build_env(&self, _port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        // Cursor uses VS Code settings, not environment variables
        vec![]
    }
    
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        // No automatic config - Cursor requires manual setup in VS Code
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn launch_command(&self, _args: &[&str]) -> Vec<String> {
        // Cursor is a VS Code extension, no command to launch
        vec![]
    }
    
    fn is_installed(&self) -> bool {
        // Cursor is always "installed" as a VS Code extension
        // We can't check if VS Code has it, but we can check if VS Code is installed
        std::process::Command::new("code")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy is running. Configure Cursor:");
        println!();
        println!("  For OpenAI-compatible models:");
        println!("    Base URL:  http://127.0.0.1:{}/v1", port);
        println!("    API Key:   your-openai-api-key");
        println!();
        println!("  For Anthropic models:");
        println!("    Base URL:  http://127.0.0.1:{}", port);
        println!("    API Key:   your-anthropic-api-key");
        println!();
        println!("  In VS Code:");
        println!("    1. Open Cursor extension settings");
        println!("    2. Go to Settings > Models > OpenAI API Key");
        println!("    3. Check 'Override OpenAI Base URL'");
        println!("    4. Set to: http://127.0.0.1:{}/v1", port);
        println!();
        println!("  For Anthropic models:");
        println!("    1. Open Cursor extension settings");
        println!("    2. Go to Settings > Models > Anthropic");
        println!("    3. Set Base URL to: http://127.0.0.1:{}", port);
    }
}

impl Default for CursorProvider {
    fn default() -> Self {
        Self
    }
}
