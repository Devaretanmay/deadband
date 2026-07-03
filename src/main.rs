// Deadband - Stop your AI agents from looping
// Phase 1: Simple proxy with exact repeat detection

use clap::{Parser, Subcommand, ArgAction};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod detector;
mod intervention;
mod proxy;
mod stats;
mod wrap;

use proxy::{ProxyState, run_proxy};
use stats::Stats;
use detector::LoopDetector;
use wrap::{provider::AgentProvider, utils, agents};

// Re-export agent providers for convenience
use agents::claude::ClaudeProvider;
use agents::aider::AiderProvider;
use agents::codex::CodexProvider;
use agents::vibe::VibeProvider;
use agents::opencode::OpenCodeProvider;
use agents::cursor::CursorProvider;
use agents::continue_dev::ContinueProvider;
use agents::cline::ClineProvider;

const DEFAULT_PORT: u16 = 4399;
const STATS_FILE: &str = "stats.json";

#[derive(Parser)]
#[command(name = "deadband")]
#[command(about = "Stop your AI agents from looping", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy
    Enable {
        /// Port to listen on (default: 4399)
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
    /// Stop the proxy
    Disable,
    /// Show proxy status and stats
    Status,
    /// Wrap an AI agent to route through Deadband proxy
    #[command(subcommand)]
    Wrap(WrapCommand),
    /// Remove Deadband wrapping from an agent
    #[command(subcommand)]
    Unwrap(UnwrapCommand),
}

#[derive(Subcommand)]
pub enum WrapCommand {
    /// Wrap Claude Code
    Claude {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
        #[arg(num_args = 0.., last = true)]
        args: Vec<String>,
    },
    /// Wrap Aider
    Aider {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
        #[arg(num_args = 0.., last = true)]
        args: Vec<String>,
    },
    /// Wrap Codex CLI
    Codex {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
        #[arg(num_args = 0.., last = true)]
        args: Vec<String>,
    },
    /// Wrap Mistral Vibe
    Vibe {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
        #[arg(num_args = 0.., last = true)]
        args: Vec<String>,
    },
    /// Wrap OpenCode
    Opencode {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
        #[arg(num_args = 0.., last = true)]
        args: Vec<String>,
    },
    /// Wrap Cursor (VS Code extension - prints setup instructions)
    Cursor {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
    },
    /// Wrap Continue (VS Code/JetBrains extension)
    Continue {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        no_proxy: bool,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
    },
    /// Wrap Cline (VS Code extension)
    Cline {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, action = ArgAction::SetTrue)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
pub enum UnwrapCommand {
    /// Remove wrapping from OpenCode
    Opencode,
    /// Remove wrapping from Continue
    Continue,
    /// Remove wrapping from Cline
    Cline,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    let stats_path = data_dir.join(STATS_FILE);
    let stats = if stats_path.exists() {
        Stats::load(&stats_path)?
    } else {
        Stats::default()
    };

    match cli.command {
        Commands::Enable { port } => {
            cmd_enable(port, data_dir, stats).await?;
        }
        Commands::Disable => {
            cmd_disable().await?;
        }
        Commands::Status => {
            cmd_status(data_dir).await?;
        }
        Commands::Wrap(wrap_cmd) => {
            cmd_wrap(wrap_cmd, data_dir)?;
        }
        Commands::Unwrap(unwrap_cmd) => {
            cmd_unwrap(unwrap_cmd)?;
        }
    }

    Ok(())
}

async fn cmd_enable(port: u16, data_dir: PathBuf, mut stats: Stats) -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Enable");
    println!("===========================");

    stats.status = "running".to_string();
    stats.save(&data_dir.join(STATS_FILE))?;

    let detector = LoopDetector::new(100); // Keep last 100 calls
    let proxy_state = ProxyState {
        port,
        detector,
        stats,
        data_dir,
    };
    let state = Arc::new(Mutex::new(proxy_state));

    println!("\n Proxy starting on port {}...", port);
    
    run_proxy(state).await?;

    Ok(())
}

async fn cmd_disable() -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Disable");
    println!("===========================");
    
    let data_dir = get_data_dir()?;
    let stats_path = data_dir.join(STATS_FILE);
    
    if stats_path.exists() {
        let mut stats = Stats::load(&stats_path)?;
        stats.status = "stopped".to_string();
        stats.save(&stats_path)?;
    }
    
    println!("\n Deadband Proxy disabled");
    Ok(())
}

async fn cmd_status(data_dir: PathBuf) -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Status");
    println!("===========================");

    let stats_path = data_dir.join(STATS_FILE);
    
    if stats_path.exists() {
        let stats = Stats::load(&stats_path)?;
        println!("\n Status: {}", stats.status);
        println!(" Total requests: {}", stats.total_requests);
        println!(" Loops detected: {}", stats.loops_detected);
        println!(" Interventions: {}", stats.interventions_applied);
        println!(" Calls prevented: {}", stats.calls_prevented);
        println!(" Credits saved: ${:.4}", stats.estimated_savings());
    } else {
        println!("\n Proxy has not been started yet.");
        println!(" Run 'deadband enable' to start.");
    }

    Ok(())
}

/// Handle wrap command
fn cmd_wrap(wrap_cmd: WrapCommand, data_dir: PathBuf) -> Result<(), anyhow::Error> {
    use WrapCommand::*;
    
    match wrap_cmd {
        Claude { port, no_proxy, verbose, args } => {
            handle_agent_wrap::<ClaudeProvider>(port, no_proxy, verbose, &args, data_dir)
        }
        Aider { port, no_proxy, verbose, args } => {
            handle_agent_wrap::<AiderProvider>(port, no_proxy, verbose, &args, data_dir)
        }
        Codex { port, no_proxy, verbose, args } => {
            handle_agent_wrap::<CodexProvider>(port, no_proxy, verbose, &args, data_dir)
        }
        Vibe { port, no_proxy, verbose, args } => {
            handle_agent_wrap::<VibeProvider>(port, no_proxy, verbose, &args, data_dir)
        }
        Opencode { port, no_proxy, verbose, args } => {
            handle_config_wrap::<OpenCodeProvider>(port, no_proxy, verbose, &args, data_dir)
        }
        Cursor { port, verbose, .. } => {
            handle_manual_wrap::<CursorProvider>(port, verbose, data_dir)
        }
        Continue { port, no_proxy, verbose, .. } => {
            handle_config_wrap::<ContinueProvider>(port, no_proxy, verbose, &[], data_dir)
        }
        Cline { port, verbose, .. } => {
            handle_config_wrap::<ClineProvider>(port, false, verbose, &[], data_dir)
        }
    }
}

/// Handle unwrap command
fn cmd_unwrap(unwrap_cmd: UnwrapCommand) -> Result<(), anyhow::Error> {
    use UnwrapCommand::*;
    
    match unwrap_cmd {
        Opencode => OpenCodeProvider.teardown_config().map_err(Into::into),
        Continue => ContinueProvider.teardown_config().map_err(Into::into),
        Cline => ClineProvider.teardown_config().map_err(Into::into),
    }
}

/// Handle agents that use environment variables only
fn handle_agent_wrap<P: AgentProvider + Default>(
    port: u16,
    no_proxy: bool,
    verbose: bool,
    args: &[String],
    _data_dir: PathBuf,
) -> Result<(), anyhow::Error> {
    let provider = P::default();
    
    if verbose {
        println!(" Deadband Wrap — {}", provider.name());
        println!("============================");
    }
    
    // Check installation
    if !provider.is_installed() {
        anyhow::bail!(
            "{} is not installed or not in PATH. Please install it first.",
            provider.name()
        );
    }
    
    if verbose {
        println!("  Found {} installed", provider.name());
    }
    
    // Start proxy if needed
    let proxy_child = if !no_proxy && !utils::check_proxy_running(port) {
        if verbose {
            println!("  Starting Deadband proxy on port {}", port);
        }
        Some(utils::start_proxy(port)?)
    } else {
        if verbose {
            if utils::check_proxy_running(port) {
                println!("  Using existing proxy on port {}", port);
            } else {
                println!("  Skipping proxy startup (--no-proxy)");
            }
        }
        None
    };
    
    // Setup configuration
    if verbose {
        println!("  Setting up configuration...");
    }
    provider.setup_config(port)?;
    
    // Build environment
    let project = utils::get_project_name();
    let env_vars = provider.build_env(port, project.as_deref());
    
    if verbose {
        println!("  Environment variables:");
        for (key, value) in &env_vars {
            println!("    {}={}", key, value);
        }
    }
    
    // Launch agent
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let cmd = provider.launch_command(&str_args);
    
    if cmd.is_empty() {
        // No command to launch - just print instructions
        if verbose {
            println!("  Setup complete:");
        }
        provider.print_setup_instructions(port);
        
        if let Some(mut child) = proxy_child {
            if verbose {
                println!("\n  Deadband proxy is running. Press Ctrl+C to stop.");
            }
            // Wait for Ctrl+C
            std::thread::park();
            child.kill()?;
        }
        
        return Ok(());
    }
    
    // Run agent with environment
    if verbose {
        println!("  Launching {} with arguments: {:?}", provider.name(), args);
    }
    
    let mut process = std::process::Command::new(&cmd[0]);
    for arg in &cmd[1..] {
        process.arg(arg);
    }
    for (key, value) in env_vars {
        process.env(key, value);
    }
    
    // Inherit stdio for interactive use
    process.stdout(std::process::Stdio::inherit());
    process.stderr(std::process::Stdio::inherit());
    process.stdin(std::process::Stdio::inherit());
    
    let status = process.status()?;
    
    // Cleanup
    if verbose {
        println!("  Cleaning up...");
    }
    provider.teardown_config()?;
    
    if let Some(mut child) = proxy_child {
        child.kill()?;
    }
    
    if !status.success() {
        anyhow::bail!("{} exited with status {:?}", provider.name(), status.code());
    }
    
    Ok(())
}

/// Handle agents that need config injection
fn handle_config_wrap<P: AgentProvider + Default>(
    port: u16,
    no_proxy: bool,
    verbose: bool,
    args: &[String],
    _data_dir: PathBuf,
) -> Result<(), anyhow::Error> {
    let provider = P::default();
    
    if verbose {
        println!(" Deadband Wrap — {}", provider.name());
        println!("============================");
    }
    
    // Check installation
    if !provider.is_installed() {
        anyhow::bail!(
            "{} is not installed. Please install it first.",
            provider.name()
        );
    }
    
    if verbose {
        println!("  Found {} installed", provider.name());
    }
    
    // Start proxy if needed
    let proxy_child = if !no_proxy && !utils::check_proxy_running(port) {
        if verbose {
            println!("  Starting Deadband proxy on port {}", port);
        }
        Some(utils::start_proxy(port)?)
    } else {
        if verbose {
            if utils::check_proxy_running(port) {
                println!("  Using existing proxy on port {}", port);
            } else {
                println!("  Skipping proxy startup (--no-proxy)");
            }
        }
        None
    };
    
    // Setup config
    if verbose {
        println!("  Injecting configuration...");
    }
    provider.setup_config(port)?;
    
    // Print setup instructions
    provider.print_setup_instructions(port);
    
    // Launch agent if applicable
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let cmd = provider.launch_command(&str_args);
    if !cmd.is_empty() {
        if verbose {
            println!("  Launching {} with arguments: {:?}", provider.name(), args);
        }
        
        let mut process = std::process::Command::new(&cmd[0]);
        for arg in &cmd[1..] {
            process.arg(arg);
        }
        
        // Inherit stdio for interactive use
        process.stdout(std::process::Stdio::inherit());
        process.stderr(std::process::Stdio::inherit());
        process.stdin(std::process::Stdio::inherit());
        
        let status = process.status()?;
        
        // Cleanup
        if verbose {
            println!("  Cleaning up...");
        }
        provider.teardown_config()?;
        
        if let Some(mut child) = proxy_child {
            child.kill()?;
        }
        
        if !status.success() {
            anyhow::bail!("{} exited with status {:?}", provider.name(), status.code());
        }
        
        return Ok(());
    }
    
    // For agents without a launch command (IDE extensions)
    if let Some(mut child) = proxy_child {
        if verbose {
            println!("\n  Deadband proxy is running. Press Ctrl+C to stop.");
        }
        // Wait for Ctrl+C
        std::thread::park();
        child.kill()?;
    }
    
    Ok(())
}

/// Handle agents that require manual setup
fn handle_manual_wrap<P: AgentProvider + Default>(
    port: u16,
    verbose: bool,
    _data_dir: PathBuf,
) -> Result<(), anyhow::Error> {
    let provider = P::default();
    
    if verbose {
        println!(" Deadband Wrap — {}", provider.name());
        println!("============================");
    }
    
    // Start proxy
    if !utils::check_proxy_running(port) {
        if verbose {
            println!("  Starting Deadband proxy on port {}", port);
        }
        utils::start_proxy(port)?;
    } else if verbose {
        println!("  Using existing proxy on port {}", port);
    }
    
    // Print instructions
    provider.print_setup_instructions(port);
    
    if verbose {
        println!("\n  Deadband proxy is running. Press Ctrl+C to stop.");
    }
    
    // Wait for Ctrl+C
    std::thread::park();
    
    Ok(())
}

fn get_data_dir() -> Result<PathBuf, anyhow::Error> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".deadband"))
}
