use std::path::PathBuf;

use clap::{Parser, Subcommand};
use loopless_core::{Orchestrator, Replayer};

#[derive(Parser)]
#[command(name = "loopless", about = "Execution runtime for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if Loopless is working
    Doctor {
        /// Path to policy config
        #[arg(short, long, default_value = "loopless.yaml")]
        config: PathBuf,
    },
    /// Start tracing execution
    Trace {
        /// Path to policy config
        #[arg(short, long, default_value = "loopless.yaml")]
        config: PathBuf,
    },
    /// Replay a saved trace
    Replay {
        /// Path to trace file
        path: PathBuf,
    },
    /// Inspect a trace in detail
    Inspect {
        /// Path to trace file
        path: PathBuf,
    },
    /// Visualize a trace as ASCII timeline
    Visualize {
        /// Path to trace file
        path: PathBuf,
    },
    /// Generate default loopless.yaml
    Init {
        /// Output path
        #[arg(short, long, default_value = "loopless.yaml")]
        output: PathBuf,
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
    }
}

fn cmd_doctor(config: &PathBuf) -> Result<(), anyhow::Error> {
    println!("Loopless Doctor");
    println!("===============");

    let config_str = match std::fs::read_to_string(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  Config: FAIL ({})", e);
            eprintln!("  Run `loopless init` to create a default config");
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

    println!("Loopless Trace — reading events from stdin (JSON lines)");
    println!("Press Ctrl+C to stop");
    println!();

    for line in std::io::stdin().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<loopless_core::ToolCallEvent>(&line) {
            Ok(event) => {
                let step = event.step;
                let (intervention, _snapshot) =
                    orch.process_with_snapshot(event, &loopless_core::AdapterCapabilities::default());
                if let Some(intervention) = intervention {
                    let kind = match intervention {
                        loopless_core::Intervention::Continue => "continue",
                        loopless_core::Intervention::Retry { .. } => "retry",
                        loopless_core::Intervention::Backoff { .. } => "backoff",
                        loopless_core::Intervention::ReplaceTool { .. } => "replace_tool",
                        loopless_core::Intervention::InjectPrompt { .. } => "inject_prompt",
                        loopless_core::Intervention::Abort { .. } => "abort",
                        loopless_core::Intervention::Custom { .. } => "custom",
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
            loopless_core::Payload::Started { .. } => "STARTED",
            loopless_core::Payload::Succeeded { .. } => "OK",
            loopless_core::Payload::Failed { .. } => "FAILED",
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
            loopless_core::Payload::Started { .. } => '.',
            loopless_core::Payload::Succeeded { .. } => '+',
            loopless_core::Payload::Failed { .. } => 'x',
        };

        let has_intervention = trace.interventions.iter().any(|r| r.event_index == i);
        let marker = if has_intervention { " !" } else { "  " };

        println!("{:3} {}{}{}", i, ".".repeat(label_len), dot, marker);
    }

    println!();
    println!("Legend: . = started  + = succeeded  x = failed  ! = intervention");
    Ok(())
}

fn cmd_init(output: &PathBuf) -> Result<(), anyhow::Error> {
    let default_config = include_str!("../../loopless.yaml");
    if output.exists() {
        eprintln!("{} already exists — not overwriting", output.display());
        std::process::exit(1);
    }
    std::fs::write(output, default_config)?;
    println!("Created default config at {}", output.display());
    Ok(())
}
