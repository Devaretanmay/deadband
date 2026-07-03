use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use deadband_proxy::config::ProxyConfig;
use deadband_proxy::discovery::ToolDiscovery;
use deadband_proxy::proxy::{ProxyState, ProxyStats};
use deadband_proxy::service::ServiceManager;

#[derive(Parser)]
#[command(name = "deadband", about = "Deadband Proxy — AI agent loop protection")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {

    Enable {

        #[arg(long)]
        persistent: bool,

        #[arg(short, long, default_value_t = 4399)]
        port: u16,

        #[arg(short, long, default_value = "deadband.yaml")]
        config: PathBuf,

        #[arg(long)]
        recover: bool,

        #[arg(long)]
        watch: Option<PathBuf>,
    },

    Disable,

    Status,

    Logs {

        #[arg(long, default_value_t = 50)]
        tail: usize,

        #[arg(long)]
        follow: bool,
    },

    Monitor {

        #[arg(short, long, default_value_t = 4399)]
        port: u16,
    },

    Set {

        #[arg(short, long)]
        port: Option<u16>,

        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    Proxy {

        #[arg(short, long, default_value_t = 4399)]
        port: u16,

        #[arg(short, long, default_value = "deadband.yaml")]
        config: PathBuf,

        #[arg(long)]
        daemon: bool,

        #[arg(long)]
        recover: bool,

        #[arg(long)]
        watch: Option<PathBuf>,
    },
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

    match cli.command {
        Commands::Enable { persistent, port, config, recover, watch } => {
            cmd_enable(persistent, port, config, recover, watch).await?
        }
        Commands::Disable => cmd_disable().await?,
        Commands::Status => cmd_status().await?,
        Commands::Logs { tail, follow } => cmd_logs(tail, follow)?,
        Commands::Monitor { port } => cmd_monitor(port).await?,
        Commands::Set { port, config } => cmd_set(port, config).await?,
        Commands::Proxy { port, config, daemon, recover, watch } => cmd_proxy(port, config, daemon, recover, watch).await?,
    }

    Ok(())
}

async fn cmd_enable(persistent: bool, port: u16, config: PathBuf, recover: bool, watch: Option<PathBuf>) -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Enable");
    println!("============================");

    let backups_dir = ProxyConfig::backups_dir();
    let discovery = ToolDiscovery::new(port, backups_dir);

    println!("\n Discovering tools...");
    let tools = discovery.enable_all()?;
    for tool in &tools {
        if tool.was_modified {
            println!("   {} configured", tool.name);
        }
    }
    if tools.is_empty() {
        println!("    No supported tools found");
    }


    println!("\n Starting proxy on port {}...", port);
    let pconfig = ProxyConfig {
        port,
        policy_path: config,
        persistent,
        recover,
        watch_dir: watch,
        ..Default::default()
    };
    let state = ProxyState::new(pconfig).await?;

    if persistent {

        let binary = std::env::current_exe()?;
        ServiceManager::install(port, &binary)?;
        println!("   System service installed (starts on boot)");
    } else {
        println!("    Use --persistent to install as a system service");
    }

    let state = Arc::new(state);
    {
        let mut stats = state.stats.lock().unwrap();
        stats.status = "running".to_string();
    }

    println!("\n Proxy is running on port {}", port);
    println!("   Use `deadband status` to check stats");
    println!("   Use `deadband disable` to stop");
    println!("   Use `deadband logs --follow` to watch activity");


    deadband_proxy::proxy::run_proxy(state).await?;

    Ok(())
}

async fn cmd_disable() -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Disable");
    println!("=============================");


    let backups_dir = ProxyConfig::backups_dir();

    let discovery = ToolDiscovery::new(4399, backups_dir);
    let tools = discovery.disable_all()?;
    for tool in &tools {
        println!("   {} config restored", tool.name);
    }


    ServiceManager::uninstall()?;
    println!("   Service stopped and uninstalled");

    println!("\n Deadband Proxy disabled");
    Ok(())
}

async fn cmd_status() -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Status");
    println!("===========================");


    let service_status = ServiceManager::status()?;
    println!("Service: {:?}", service_status);


    let stats_path = ProxyConfig::data_dir().join("stats.json");
    if stats_path.exists() {
        let content = std::fs::read_to_string(&stats_path)?;
        let stats: ProxyStats = serde_json::from_str(&content)?;
        println!("\n Statistics:");
        println!("  Total requests:     {}", stats.total_requests);
        println!("  Loops detected:     {}", stats.loops_detected);
        println!("  Interventions:      {}", stats.interventions_applied);
        println!("  Calls prevented:    {}", stats.calls_prevented);
        println!("  Estimated savings:  ${:.4}", stats.estimated_savings);
        println!("  Uptime:             {}", stats.start_time);
        println!("  Status:             {}", stats.status);
    } else {
        println!("\n  No statistics available — proxy has not been started yet.");
    }


    match deadband_proxy::discovery::validate_proxy(4399) {
        Ok(_) => println!("\n   Proxy is reachable on port 4399"),
        Err(e) => println!("\n    Proxy check: {}", e),
    }

    Ok(())
}

fn cmd_logs(tail: usize, follow: bool) -> Result<(), anyhow::Error> {
    let log_file = ProxyConfig::log_file();
    if !log_file.exists() {
        println!("No log file found at {:?}", log_file);
        return Ok(());
    }

    if follow {

        let status = std::process::Command::new("tail")
            .args(["-f", &log_file.to_string_lossy()])
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run tail: {}", e))?;
        if !status.success() {
            eprintln!("tail failed with status: {}", status);
        }
    } else {

        let output = std::process::Command::new("tail")
            .args(["-n", &tail.to_string(), &log_file.to_string_lossy()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run tail: {}", e))?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    Ok(())
}

async fn cmd_monitor(port: u16) -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Live Monitor");
    println!("=================================");


    let stats_path = ProxyConfig::data_dir().join("stats.json");

    loop {

        print!("\x1B[2J\x1B[1;1H");

        println!(" Deadband Proxy Monitor (port {})", port);
        println!("   Press Ctrl+C to exit");
        println!();

        if stats_path.exists() {
            let content = std::fs::read_to_string(&stats_path).unwrap_or_default();
            if let Ok(stats) = serde_json::from_str::<ProxyStats>(&content) {
                println!("  Requests:     {}", stats.total_requests);
                println!("  Loops:        {}", stats.loops_detected);
                println!("  Interventions: {}", stats.interventions_applied);
                println!("  Prevented:    {}", stats.calls_prevented);
                println!("  Savings:      ${:.4}", stats.estimated_savings);
                println!("  Status:       {}", stats.status);


                print!("\n  Loops: ");
                for _ in 0..stats.loops_detected.min(50) {
                    print!("█");
                }
                println!(" {}", stats.loops_detected);

                print!("  Interventions: ");
                for _ in 0..stats.interventions_applied.min(50) {
                    print!("█");
                }
                println!(" {}", stats.interventions_applied);
            }
        } else {
            println!("  Waiting for proxy to start...");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

async fn cmd_set(
    port: Option<u16>,
    config: Option<PathBuf>,
) -> Result<(), anyhow::Error> {
    println!(" Deadband Proxy — Settings");
    println!("=============================");

    let config_path = ProxyConfig::data_dir().join("config.json");
    let mut current = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str::<serde_json::Value>(&content).unwrap_or_default()
    } else {
        serde_json::json!({})
    };

    if let Some(p) = port {
        current["port"] = serde_json::json!(p);
        println!("  Port set to {}", p);
    }
    if let Some(c) = config {
        current["policy_path"] = serde_json::json!(c.to_string_lossy().to_string());
        println!("  Config set to {:?}", c);
    }

    std::fs::write(&config_path, serde_json::to_string_pretty(&current)?)?;
    println!("  Settings saved to {:?}", config_path);


    println!("    Restart the proxy to apply changes");

    Ok(())
}

async fn cmd_proxy(port: u16, config: PathBuf, daemon: bool, recover: bool, watch: Option<PathBuf>) -> Result<(), anyhow::Error> {
    if daemon {

        let log_dir = ProxyConfig::log_dir();
        tokio::fs::create_dir_all(&log_dir).await?;
        tracing::info!("Starting Deadband Proxy daemon on port {}", port);
    }

    let pconfig = ProxyConfig {
        port,
        policy_path: config,
        recover,
        watch_dir: watch,
        ..Default::default()
    };

    let state = Arc::new(ProxyState::new(pconfig).await?);
    deadband_proxy::proxy::run_proxy(state).await?;

    Ok(())
}
