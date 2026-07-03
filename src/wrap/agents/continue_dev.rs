// Continue provider for Deadband
// Continue is a VS Code/JetBrains extension that uses config files

use crate::wrap::{provider::AgentProvider, utils};

/// Provider for Continue (VS Code/JetBrains extension)
/// Uses config file injection to add Deadband system message
pub struct ContinueProvider;

// Default Continue config directory
const CONTINUE_DIR: &str = ".continue";
const CONFIG_FILE: &str = "config.json";

impl AgentProvider for ContinueProvider {
    fn id(&self) -> &str { "continue" }
    
    fn name(&self) -> &str { "Continue" }
    
    fn build_env(&self, _port: u16, _project: Option<&str>) -> Vec<(&str, String)> {
        vec![]
    }
    
    fn setup_config(&self, port: u16) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let config_dir = current_dir.join(CONTINUE_DIR);
        let config_path = config_dir.join(CONFIG_FILE);
        
        // Create backup on first injection
        utils::backup_file(&config_path)?;
        
        // Ensure directory exists
        utils::ensure_dir(&config_dir)?;
        
        // Read existing config
        let mut config: serde_json::Value = if config_path.exists() {
            utils::read_json(&config_path)?
        } else {
            serde_json::json!({
                "models": [],
                "systemMessage": ""
            })
        };
        
        // Build Deadband system message
        let deadband_msg = format!(
            "\n\n---\n[Deadband Loop Detector Active - Port {}]\n\n",
            port
        );
        
        // Append to systemMessage if it exists and is a string
        if let Some(system_msg) = config.get_mut("systemMessage") {
            if let Some(s) = system_msg.as_str() {
                let new_msg = format!("{}{}", s, deadband_msg.clone());
                *system_msg = serde_json::Value::String(new_msg);
            }
            // If systemMessage is not a string, leave it alone
        } else {
            config["systemMessage"] = serde_json::Value::String(deadband_msg.clone());
        }
        
        // Also handle per-model systemMessage
        if let Some(models) = config.get_mut("models") {
            if let Some(models_array) = models.as_array_mut() {
                for model in models_array {
                    if let Some(model_obj) = model.as_object_mut() {
                        if model_obj.get("systemMessage").is_none() {
                            model_obj.insert("systemMessage".to_string(), 
                                serde_json::Value::String(deadband_msg.clone()));
                        } else if let Some(model_msg) = model_obj.get_mut("systemMessage") {
                            if let Some(s) = model_msg.as_str() {
                                let new_msg = format!("{}{}", s, deadband_msg.clone());
                                *model_msg = serde_json::Value::String(new_msg);
                            }
                        }
                    }
                }
            }
        }
        
        // Write back
        utils::write_json(&config_path, &config)?;
        
        println!("  Continue config updated with Deadband system message");
        
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let config_path = current_dir.join(CONTINUE_DIR).join(CONFIG_FILE);
        let backup_file = config_path.with_extension("json.deadband-backup");
        
        if backup_file.exists() {
            utils::restore_file(&backup_file)?;
            println!("  Continue config restored from backup");
        } else {
            println!("  No Continue backup found, nothing to restore");
        }
        
        Ok(())
    }
    
    fn launch_command(&self, _args: &[&str]) -> Vec<String> {
        // Continue is an IDE extension, no command to launch
        vec![]
    }
    
    fn is_installed(&self) -> bool {
        // Check if .continue directory exists with config.json
        let current_dir = std::env::current_dir().ok();
        current_dir
            .map(|dir| dir.join(CONTINUE_DIR).join(CONFIG_FILE).exists())
            .unwrap_or(false)
    }
    
    fn print_setup_instructions(&self, port: u16) {
        println!("  Deadband proxy configured for Continue");
        println!();
        println!("  System message updated in .continue/config.json");
        println!("  Open VS Code or JetBrains IDE and use Continue extension.");
        println!("  Continue will now route requests through Deadband on port {}", port);
    }
}

impl Default for ContinueProvider {
    fn default() -> Self {
        Self
    }
}
