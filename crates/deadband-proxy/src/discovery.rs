use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct ToolConfig {
    pub name: String,
    pub config_path: PathBuf,
    pub backup_path: PathBuf,
    pub was_modified: bool,
}

pub struct ToolDiscovery {
    port: u16,
    backups_dir: PathBuf,
}

impl ToolDiscovery {
    pub fn new(port: u16, backups_dir: PathBuf) -> Self {
        Self { port, backups_dir }
    }


    pub fn discover_all(&self) -> Vec<ToolConfig> {
        let mut tools = Vec::new();

        if let Some(t) = self.discover_aider() {
            tools.push(t);
        }
        if let Some(t) = self.discover_claude_code() {
            tools.push(t);
        }
        if let Some(t) = self.discover_cursor() {
            tools.push(t);
        }
        if let Some(t) = self.discover_continue() {
            tools.push(t);
        }
        if let Some(t) = self.discover_copilot_cli() {
            tools.push(t);
        }
        if let Some(t) = self.discover_opencode() {
            tools.push(t);
        }

        tools
    }


    pub fn enable_all(&self) -> Result<Vec<ToolConfig>> {
        let tools = self.discover_all();
        let mut enabled = Vec::new();

        for tool in &tools {
            match self.enable_tool(tool) {
                Ok(config) => enabled.push(config),
                Err(e) => {
                    tracing::warn!("Failed to enable {}: {}", tool.name, e);
                }
            }
        }

        Ok(enabled)
    }


    pub fn disable_all(&self) -> Result<Vec<ToolConfig>> {
        let tools = self.discover_all();
        let mut disabled = Vec::new();

        for tool in &tools {
            match self.disable_tool(&tool.name) {
                Ok(config) => disabled.push(config),
                Err(e) => {
                    tracing::warn!("Failed to disable {}: {}", tool.name, e);
                }
            }
        }

        Ok(disabled)
    }

    fn proxy_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }


    fn discover_aider(&self) -> Option<ToolConfig> {
        let home = dirs::home_dir()?;
        let paths = vec![
            home.join(".aider.conf.yml"),
            home.join(".aider.conf.yaml"),
            PathBuf::from(".aider.conf.yml"),
        ];
        for path in &paths {
            if path.exists() {
                let name = format!("aider_{}", path.file_stem()?.to_str()?);
                return Some(ToolConfig {
                    name,
                    config_path: path.clone(),
                    backup_path: self.backups_dir.join(format!("aider_{}.bak", path.file_name()?.to_str()?)),
                    was_modified: false,
                });
            }
        }
        None
    }


    fn discover_claude_code(&self) -> Option<ToolConfig> {
        let home = dirs::home_dir()?;
        let path = home.join(".claude").join("settings.json");
        if path.exists() {
            Some(ToolConfig {
                name: "claude_code".into(),
                config_path: path,
                backup_path: self.backups_dir.join("claude_code_settings.json.bak"),
                was_modified: false,
            })
        } else {
            None
        }
    }


    fn discover_cursor(&self) -> Option<ToolConfig> {
        let home = dirs::home_dir()?;

        let mac_path = home.join("Library").join("Application Support").join("Cursor").join("User").join("settings.json");
        if mac_path.exists() {
            return Some(ToolConfig {
                name: "cursor".into(),
                config_path: mac_path,
                backup_path: self.backups_dir.join("cursor_settings.json.bak"),
                was_modified: false,
            });
        }

        let linux_path = home.join(".config").join("Cursor").join("User").join("settings.json");
        if linux_path.exists() {
            return Some(ToolConfig {
                name: "cursor".into(),
                config_path: linux_path,
                backup_path: self.backups_dir.join("cursor_settings.json.bak"),
                was_modified: false,
            });
        }
        None
    }


    fn discover_continue(&self) -> Option<ToolConfig> {
        let home = dirs::home_dir()?;
        let path = home.join(".continue").join("config.json");
        if path.exists() {
            Some(ToolConfig {
                name: "continue".into(),
                config_path: path,
                backup_path: self.backups_dir.join("continue_config.json.bak"),
                was_modified: false,
            })
        } else {
            None
        }
    }


    fn discover_opencode(&self) -> Option<ToolConfig> {
        let home = dirs::home_dir()?;
        let path = home.join(".config").join("opencode").join("opencode.json");
        if path.exists() {
            Some(ToolConfig {
                name: "opencode".into(),
                config_path: path,
                backup_path: self.backups_dir.join("opencode.json.bak"),
                was_modified: false,
            })
        } else {
            None
        }
    }


    fn discover_copilot_cli(&self) -> Option<ToolConfig> {


        let copilot_paths = vec![
            PathBuf::from("/usr/local/bin/github-copilot-cli"),
            PathBuf::from("/opt/homebrew/bin/github-copilot-cli"),
        ];
        for path in &copilot_paths {
            if path.exists() {
                return Some(ToolConfig {
                    name: "copilot_cli".into(),
                    config_path: path.clone(),
                    backup_path: self.backups_dir.join("copilot_cli.bak"),
                    was_modified: false,
                });
            }
        }
        None
    }


    fn enable_tool(&self, tool: &ToolConfig) -> Result<ToolConfig> {

        if tool.config_path.exists() {
            std::fs::create_dir_all(&self.backups_dir)
                .with_context(|| format!("Failed to create backups dir: {:?}", self.backups_dir))?;
            std::fs::copy(&tool.config_path, &tool.backup_path)
                .with_context(|| format!("Failed to backup config: {:?}", tool.config_path))?;
            tracing::info!("Backed up {} to {:?}", tool.name, tool.backup_path);
        }

        match tool.name.as_str() {
            "claude_code" => self.enable_claude_code(tool)?,
            "cursor" => self.enable_cursor(tool)?,
            "continue" => self.enable_continue(tool)?,
            "opencode" => self.enable_opencode(tool)?,
            _ => {


                self.enable_env_based(tool)?;
            }
        }

        Ok(ToolConfig {
            was_modified: true,
            ..tool.clone()
        })
    }

    fn enable_claude_code(&self, tool: &ToolConfig) -> Result<()> {
        let content = std::fs::read_to_string(&tool.config_path)
            .with_context(|| format!("Failed to read {}", tool.config_path.display()))?;

        let mut settings: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
            .unwrap_or_else(|_| HashMap::new());

        settings.insert(
            "anthropic_base_url".to_string(),
            serde_json::Value::String(self.proxy_url()),
        );

        let new_content = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&tool.config_path, new_content)
            .with_context(|| format!("Failed to write {}", tool.config_path.display()))?;

        tracing::info!("Enabled Deadband Proxy for Claude Code");
        Ok(())
    }

    fn enable_cursor(&self, tool: &ToolConfig) -> Result<()> {
        let content = std::fs::read_to_string(&tool.config_path)
            .with_context(|| format!("Failed to read {}", tool.config_path.display()))?;

        let mut settings: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
            .unwrap_or_else(|_| HashMap::new());

        settings.insert(
            "openai_base_url".to_string(),
            serde_json::Value::String(self.proxy_url()),
        );

        let new_content = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&tool.config_path, new_content)
            .with_context(|| format!("Failed to write {}", tool.config_path.display()))?;

        tracing::info!("Enabled Deadband Proxy for Cursor");
        Ok(())
    }

    fn enable_continue(&self, _tool: &ToolConfig) -> Result<()> {

        tracing::info!("Deadband Proxy for Continue: manual configuration may be needed");
        Ok(())
    }

    fn enable_opencode(&self, tool: &ToolConfig) -> Result<()> {
        let content = std::fs::read_to_string(&tool.config_path)
            .with_context(|| format!("Failed to read {}", tool.config_path.display()))?;

        let mut config: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse OpenCode config: {}", tool.config_path.display()))?;



        if let Some(providers) = config.get_mut("provider").and_then(|p| p.as_object_mut()) {
            for (_name, provider) in providers.iter_mut() {
                if let Some(options) = provider.get_mut("options").and_then(|o| o.as_object_mut()) {
                    if let Some(base_url) = options.get_mut("baseURL") {

                        let original_url = base_url.as_str().unwrap_or("").to_string();

                        if !original_url.is_empty() && original_url != self.proxy_url() && !original_url.contains("localhost:4399") {
                            let upstream_path = crate::config::ProxyConfig::data_dir().join("upstream_url.txt");
                            let _ = std::fs::write(&upstream_path, &original_url);
                        }

                        *base_url = serde_json::Value::String(self.proxy_url());
                    }
                }
            }
        }

        let new_content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&tool.config_path, new_content)
            .with_context(|| format!("Failed to write {}", tool.config_path.display()))?;

        tracing::info!("Enabled Deadband Proxy for OpenCode");
        Ok(())
    }

    fn enable_env_based(&self, _: &ToolConfig) -> Result<()> {

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let proxy_url = self.proxy_url();


        for rc_file in &[".zshrc", ".bashrc", ".bash_profile"] {
            let rc_path = home.join(rc_file);
            if rc_path.exists() {
                let content = std::fs::read_to_string(&rc_path).unwrap_or_default();
                let export_openai = format!("\nexport OPENAI_BASE_URL={}\n", proxy_url);
                let export_anthropic = format!("\nexport ANTHROPIC_BASE_URL={}\n", proxy_url);

                if !content.contains("OPENAI_BASE_URL=localhost:4399") {
                    std::fs::write(&rc_path, format!("{}{}", content, export_openai))
                        .ok();
                }
                if !content.contains("ANTHROPIC_BASE_URL=localhost:4399") {
                    std::fs::write(&rc_path, format!("{}{}", content, export_anthropic))
                        .ok();
                }
                tracing::info!("Added Deadband Proxy env vars to {}", rc_path.display());
                break;
            }
        }

        Ok(())
    }


    fn disable_tool(&self, tool_name: &str) -> Result<ToolConfig> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        let config_path = match tool_name {
            "claude_code" => home.join(".claude").join("settings.json"),
            "cursor" => {
                let mac = home.join("Library").join("Application Support").join("Cursor").join("User").join("settings.json");
                if mac.exists() { mac } else { home.join(".config").join("Cursor").join("User").join("settings.json") }
            },
            "continue" => home.join(".continue").join("config.json"),
            "opencode" => home.join(".config").join("opencode").join("opencode.json"),
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };

        let backup_path = self.backups_dir.join(format!("{}.bak", tool_name));

        if backup_path.exists() {
            std::fs::copy(&backup_path, &config_path)
                .with_context(|| format!("Failed to restore config for {}", tool_name))?;
            std::fs::remove_file(&backup_path).ok();
            tracing::info!("Restored config for {} from backup", tool_name);
        }

        Ok(ToolConfig {
            name: tool_name.to_string(),
            config_path,
            backup_path,
            was_modified: true,
        })
    }
}

pub fn validate_proxy(port: u16) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let resp = client
        .get(format!("http://localhost:{}/v1/chat/completions", port))
        .send()?;

    if resp.status().is_success() || resp.status().as_u16() == 400 {

        Ok(())
    } else {
        Err(anyhow::anyhow!("Proxy returned status: {}", resp.status()))
    }
}
