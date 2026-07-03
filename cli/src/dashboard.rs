use deadband_core::RecoveryMetrics;

pub fn print_snapshot(metrics: &RecoveryMetrics) {
    println!("=== Deadband Agent Metrics ===");
    println!("  Interventions:  {}", metrics.intervention_count);
    println!("  Calls Prevented: {}", metrics.prevented_calls);
    println!("  Recovery Time:   {}ms", metrics.recovery_time_ms);
    println!("  Total Events:    {}", metrics.events.len());
    println!("  Breakdown:");
    for (kind, count) in &metrics.detection_breakdown {
        println!("    {}: {}", kind, count);
    }
}
