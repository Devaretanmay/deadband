// Cline provider for Deadband
// Cline is a VS Code extension that uses .clinerules file

use std::fs;
use crate::wrap::{provider::AgentProvider, utils};

/// Provider for Cline (VS Code extension)
/// Uses .clinerules file injection for guidance
pub struct ClineProvider;

// Marker comments for idempotent injection
const MARKER_START: &str = "<!-- deadband:guidance -->";
const MARKER_END: &str = "<!-- /deadband:guidance -->";

const CLINERULES_FILE: &str = ".clinerules";

impl AgentProvider for ClineProvider {
    fn id(&self) -> &str { "cline" }
    
    fn name(&self) -> &str { "Cline" }
    
    fn build_env(&self, _port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        vec![]
    }
    
    fn setup_config(&self, port: u16) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let clinerules_path = current_dir.join(CLINERULES_FILE);
        
        // Create backup on first injection
        utils::backup_file(&clinerules_path)?;
        
        // Build guidance HTML
        let guidance = format!(
            r#"{}
<system>
You are configured to use Deadband Proxy at http://127.0.0.1:{}/v1
This provides loop detection and intervention to prevent agent oscillation.
Your requests will be monitored for repeating patterns and prevented if necessary.
</system>
{}
"#, 
            MARKER_START, port, MARKER_END
        );
        
        // Read existing content
        let content = if clinerules_path.exists() {
            let existing = fs::read_to_string(&clinerules_path)?;
            // Remove existing markers if present
            if existing.contains(MARKER_START) {
                existing
            } else {
                format!("{}\n\n{}", existing, guidance)
            }
        } else {
            guidance
        };
        
        // Write back
        fs::write(&clinerules_path, content)?;
        
        println!("  Cline guidance injected into .clinerules");
        
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let clinerules_path = current_dir.join(CLINERULES_FILE);
        let backup_file = clinerules_path.with_extension("deadband-backup");
        
        if backup_file.exists() {
            utils::restore_file(&backup_file)?;
            println!("  Cline .clinerules restored from backup");
        } else {
            println!("  No Cline backup found, nothing to restore");
        }
        
        Ok(())
    }
    
    fn launch_command(&self, _args: &[&str]) -> Vec<String> {
        // Cline is a VS Code extension, no command to launch
        vec![]
    }
    
    fn is_installed(&self) -> bool {
        // Check if VS Code is installed
        std::process::Command::new("code")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy is running. Configure Cline:");
        println!();
        println!("  Guidance has been injected into .clinerules");
        println!("  You need to configure the API base URL in Cline:");
        println!();
        println!("  In VS Code:");
        println!("    1. Open Cline extension settings");
        println!("    2. Go to Settings > Cline > API Provider");
        println!("    3. Set Anthropic Base URL to: http://127.0.0.1:{}", port);
        println!("    4. Set OpenAI Compatible Base URL to: http://127.0.0.1:{}/v1", port);
        println!();
        println!("  Cline will use the guidance from .clinerules automatically.");
    }
}

impl Default for ClineProvider {
    fn default() -> Self {
        Self
    }
}
