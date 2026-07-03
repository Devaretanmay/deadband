// OpenCode provider for Deadband
// Uses config file injection to add Deadband proxy as a provider

use crate::wrap::{provider::AgentProvider, utils};

/// Provider for OpenCode CLI
/// Uses config file injection to configure the proxy
pub struct OpenCodeProvider;

// Marker comments for idempotent config injection
const PROVIDER_MARKER_START: &str = "// --- Deadband proxy provider ---";
const PROVIDER_MARKER_END: &str = "// --- end Deadband proxy provider ---";

impl AgentProvider for OpenCodeProvider {
    fn id(&self) -> &str { "opencode" }
    
    fn name(&self) -> &str { "OpenCode" }
    
    fn build_env(&self, _port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        // OpenCode reads config from file, no env vars needed
        vec![]
    }
    
    fn setup_config(&self, port: u16) -> anyhow::Result<()> {
        let config_dir = utils::get_config_dir("opencode")?;
        let config_file = config_dir.join("opencode.json");
        
        // Create backup on first injection
        utils::backup_file(&config_file)?;
        
        // Ensure directory exists
        utils::ensure_dir(&config_dir)?;
        
        // Read existing config
        let mut config: serde_json::Value = if config_file.exists() {
            utils::read_json(&config_file)?
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        
        // Build Deadband provider config
        let deadband_provider = serde_json::json!({
            "headroom": {
                "npm": "@ai-sdk/openai-compatible",
                "name": "Deadband Proxy",
                "options": {
                    "baseURL": format!("http://127.0.0.1:{}/v1", port)
                }
            }
        });
        
        // Inject provider into config
        if let Some(obj) = config.as_object_mut() {
            // Remove any existing deadband provider first
            obj.remove("provider");
            
            // Add new provider config
            obj.insert("provider".to_string(), deadband_provider);
        }
        
        // Write back
        utils::write_json(&config_file, &config)?;
        
        println!("  OpenCode config updated with Deadband proxy");
        
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        let config_dir = utils::get_config_dir("opencode")?;
        let config_file = config_dir.join("opencode.json");
        let backup_file = config_file.with_extension("json.deadband-backup");
        
        if backup_file.exists() {
            utils::restore_file(&backup_file)?;
            println!("  OpenCode config restored from backup");
        } else {
            println!("  No OpenCode backup found, nothing to restore");
        }
        
        Ok(())
    }
    
    fn launch_command(&self, args: &[&str]) -> Vec<String> {
        let mut cmd = vec!["opencode".to_string()];
        cmd.extend(args.iter().map(|s| s.to_string()));
        cmd
    }
    
    fn is_installed(&self) -> bool {
        std::process::Command::new("opencode")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy configured in OpenCode");
        println!();
        println!("  OpenCode will now route requests through Deadband on port {}", port);
        println!("  Run OpenCode with your desired arguments.");
    }
}

impl Default for OpenCodeProvider {
    fn default() -> Self {
        Self
    }
}
