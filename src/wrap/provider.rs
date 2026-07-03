// Agent Provider Trait for Deadband
// Defines the interface for agent-specific integration

/// Trait for agent-specific integration with Deadband proxy
pub trait AgentProvider {
    /// Provider identifier (e.g., "claude", "aider")
    fn id(&self) -> &str;
    
    /// Display name for UI
    fn name(&self) -> &str;
    
    /// Build environment variables for agent process
    /// Returns list of (key, value) pairs to set
    fn build_env(&self, port: u16, project: Option<&str>) -> Vec<(&str, String)>;
    
    /// Setup persistent configuration (config injection)
    /// Called before launching the agent
    /// Override for agents that need config file modification
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Teardown/restore configuration
    /// Called after agent exits or on unwrap
    /// Override for agents that need config cleanup
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Get command to launch the agent
    /// Returns command and arguments as a vector of strings
    /// Return empty vector for agents that don't launch a process
    /// (e.g., IDE extensions that require manual setup)
    fn launch_command(&self, args: &[&str]) -> Vec<String>;
    
    /// Check if agent binary is available in PATH
    fn is_installed(&self) -> bool;
    
    /// Print manual setup instructions (for agents without auto-config)
    /// Default implementation does nothing
    fn print_setup_instructions(&self, port: u16) {
        println!("Configure {} to use proxy at port {}", self.name(), port);
    }
}

/// Default implementation for providers that just need env vars
pub struct SimpleAgentProvider {
    id: &'static str,
    name: &'static str,
    binary_name: &'static str,
    env_vars: Vec<(&'static str, fn(u16, Option<&str>) -> String)>,
}

impl SimpleAgentProvider {
    pub fn new(
        id: &'static str,
        name: &'static str,
        binary_name: &'static str,
        env_vars: Vec<(&'static str, fn(u16, Option<&str>) -> String)>,
    ) -> Self {
        Self {
            id,
            name,
            binary_name,
            env_vars,
        }
    }
}

impl AgentProvider for SimpleAgentProvider {
    fn id(&self) -> &str { self.id }
    fn name(&self) -> &str { self.name }
    
    fn build_env(&self, port: u16, project: Option<&str>) -> Vec<(&str, String)> {
        self.env_vars.iter()
            .map(|(key, value_fn)| (*key, value_fn(port, project)))
            .collect()
    }
    
    fn launch_command(&self, args: &[&str]) -> Vec<String> {
        let mut cmd = vec![self.binary_name.to_string()];
        cmd.extend(args.iter().map(|s| s.to_string()));
        cmd
    }
    
    fn is_installed(&self) -> bool {
        std::process::Command::new(self.binary_name)
            .arg("--version")
            .output()
            .is_ok()
    }
    
    fn setup_config(&self, _port: u16) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn teardown_config(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
