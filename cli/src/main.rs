use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod dashboard;

use deadband_core::{Orchestrator, Replayer};

#[derive(Parser)]
#[command(name = "deadband", about = "Execution runtime for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {

    Doctor {

        #[arg(short, long, default_value = "deadband.yaml")]
        config: PathBuf,
    },

    Trace {

        #[arg(short, long, default_value = "deadband.yaml")]
        config: PathBuf,
    },

    Replay {

        path: PathBuf,
    },

    Inspect {

        path: PathBuf,
    },

    Visualize {

        path: PathBuf,
    },

    Init {

        #[arg(short, long, default_value = "deadband.yaml")]
        output: PathBuf,
    },

    Dashboard {

        #[arg(short, long, default_value = "deadband.yaml")]
        config: PathBuf,

        #[arg(long)]
        snapshot: bool,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor { config } => cmd_doctor(&config),
        Commands::Trace { config } => cmd_trace(&config),
        Commands::Replay { path } => cmd_replay(&path),
        Commands::Inspect { path } => cmd_inspect(&path),
        Commands::Visualize { path } => cmd_visualize(&path),
        Commands::Init { output } => cmd_init(&output),
        Commands::Dashboard { config, snapshot } => cmd_dashboard(&config, snapshot),
    }
}

fn cmd_doctor(config: &PathBuf) -> Result<(), anyhow::Error> {
    println!("Deadband Doctor");
    println!("===============");

    let config_str = match std::fs::read_to_string(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  Config: FAIL ({})", e);
            eprintln!("  Run `deadband init` to create a default config");
            std::process::exit(1);
        }
    };
    println!("  Config: OK ({} loaded)", config.display());

    match Orchestrator::from_yaml(&config_str) {
            Ok(orch) => {
                println!("  Core:   OK ({} policies, {} detectors)", orch.policy_count(), orch.detector_count());
            }
        Err(e) => {
            eprintln!("  Core:   FAIL ({})", e);
            std::process::exit(1);
        }
    }

    match reqwest::blocking::get("http://localhost:8081") {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("  Sidecar: OK");
            } else {
                println!("  Sidecar: WARN (unexpected response)");
            }
        }
        Err(_) => {
            println!("  Sidecar: WARN (not running — semantic detection disabled)");
        }
    }

    Ok(())
}

fn cmd_trace(config: &PathBuf) -> Result<(), anyhow::Error> {
    let config_str = std::fs::read_to_string(config)?;
    let mut orch = Orchestrator::from_yaml(&config_str)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Deadband Trace — reading events from stdin (JSON lines)");
    println!("Press Ctrl+C to stop");
    println!();

    for line in std::io::stdin().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<deadband_core::ToolCallEvent>(&line) {
            Ok(event) => {
                let step = event.step;
                let (intervention, _snapshot) =
                    orch.process_with_snapshot(event, &deadband_core::AdapterCapabilities::default());
                if let Some(intervention) = intervention {
                    let kind = match intervention {
                        deadband_core::Intervention::Continue => "continue",
                        deadband_core::Intervention::Retry { .. } => "retry",
                        deadband_core::Intervention::Backoff { .. } => "backoff",
                        deadband_core::Intervention::ReplaceTool { .. } => "replace_tool",
                        deadband_core::Intervention::InjectPrompt { .. } => "inject_prompt",
                        deadband_core::Intervention::Abort { .. } => "abort",
                        deadband_core::Intervention::Custom { .. } => "custom",
                    };
                    println!("[{}] Intervention: {}", step, kind);
                }
            }
            Err(e) => {
                eprintln!("Skipping invalid line: {}", e);
            }
        }
    }

    Ok(())
}

fn cmd_replay(path: &PathBuf) -> Result<(), anyhow::Error> {
    let trace = Replayer::from_json(path)?;
    println!("Trace: {} ({} events, {} interventions)",
        trace.execution_id,
        trace.events.len(),
        trace.interventions.len(),
    );
    println!("  Started:  {}", trace.started_at);
    println!("  Loops prevented: {}", trace.metrics.prevented_calls);
    println!("  Recovery time:   {}ms", trace.metrics.recovery_time_ms);
    Ok(())
}

fn cmd_inspect(path: &PathBuf) -> Result<(), anyhow::Error> {
    let trace = Replayer::from_json(path)?;
    println!("Execution ID: {}", trace.execution_id);
    println!("Started:      {}", trace.started_at);
    println!("Events:       {}", trace.events.len());
    println!("Interventions: {}", trace.interventions.len());
    println!("Prevented:    {}", trace.metrics.prevented_calls);
    println!("Recovery:     {}ms", trace.metrics.recovery_time_ms);
    println!();
    println!("Events:");
    for (i, event) in trace.events.iter().enumerate() {
        let status = match event.payload {
            deadband_core::Payload::Started { .. } => "STARTED",
            deadband_core::Payload::Succeeded { .. } => "OK",
            deadband_core::Payload::Failed { .. } => "FAILED",
        };
        println!("  {:3}. [{}] {} {}", i, status, event.tool_name, event.arguments);
    }
    println!();
    println!("Interventions:");
    for record in &trace.interventions {
        println!("  Event {}: {:?}", record.event_index, record.intervention);
    }
    Ok(())
}

fn cmd_visualize(path: &PathBuf) -> Result<(), anyhow::Error> {
    let trace = Replayer::from_json(path)?;
    let timeline_len = 60usize;

    println!("Timeline ({} events):", trace.events.len());
    println!();

    for (i, event) in trace.events.iter().enumerate() {
        let label = event.tool_name.to_string();
        let label_len = label.len().min(timeline_len.saturating_sub(10));
        let dot = match event.payload {
            deadband_core::Payload::Started { .. } => '.',
            deadband_core::Payload::Succeeded { .. } => '+',
            deadband_core::Payload::Failed { .. } => 'x',
        };

        let has_intervention = trace.interventions.iter().any(|r| r.event_index == i);
        let marker = if has_intervention { " !" } else { "  " };

        println!("{:3} {}{}{}", i, ".".repeat(label_len), dot, marker);
    }

    println!();
    println!("Legend: . = started  + = succeeded  x = failed  ! = intervention");
    Ok(())
}

fn cmd_dashboard(config: &PathBuf, snapshot: bool) -> Result<(), anyhow::Error> {
    let config_str = std::fs::read_to_string(config)?;
    let orch = Orchestrator::from_yaml(&config_str)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let metrics = orch.metrics();

    if snapshot {
        crate::dashboard::print_snapshot(metrics);
    } else {
        eprintln!("Interactive dashboard not available. Use --snapshot for one-shot view.");
    }

    Ok(())
}

fn cmd_init(output: &PathBuf) -> Result<(), anyhow::Error> {
    let default_config = include_str!("../deadband.yaml");
    if output.exists() {
        eprintln!("{} already exists — not overwriting", output.display());
        std::process::exit(1);
    }
    std::fs::write(output, default_config)?;
    println!("Created default config at {}", output.display());
    Ok(())
}
