use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "omega-point", about = "Convergence detector — monitors system stability and coherence")]
pub struct Cli {
    /// Echo-X master address
    #[arg(long = "echo-x", default_value = "127.0.0.1:8888")]
    echo_x: String,

    /// Monitoring duration in seconds
    #[arg(long, default_value = "10")]
    duration: u64,

    /// Polling interval in seconds
    #[arg(long, default_value = "2")]
    interval: u64,
}

/// Convergence measurement point
#[derive(Debug, Serialize)]
pub struct ConvergencePoint {
    pub timestamp: String,
    pub measurement_id: u32,
    pub stability_index: f64,
    pub coherence_level: f64,
    pub resonance_frequency: f64,
}

/// Overall convergence analysis result
#[derive(Debug, Serialize)]
pub struct ConvergenceResult {
    pub timestamp: String,
    pub total_measurements: usize,
    pub convergence_points: Vec<ConvergencePoint>,
    pub stability_average: f64,
    pub coherence_average: f64,
    pub system_status: String,
    pub converged: bool,
    pub convergence_confidence: f64,
}

/// Measure system coherence by analyzing process and thread stability
fn measure_coherence() -> (f64, f64, f64) {
    use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};

    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    std::thread::sleep(Duration::from_millis(100));
    sys.refresh_cpu();
    sys.refresh_memory();

    // Stability Index: inverse of CPU variance (smooth = stable)
    let cpu_usages: Vec<f64> = sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect();
    let avg_cpu = cpu_usages.iter().sum::<f64>() / cpu_usages.len().max(1) as f64;
    let variance = cpu_usages.iter()
        .map(|u| (u - avg_cpu).powi(2))
        .sum::<f64>() / cpu_usages.len().max(1) as f64;
    let stability = 1.0 / (1.0 + variance.sqrt()); // Higher stability = lower variance

    // Coherence Level: how balanced memory and CPU are
    let mem_total = sys.total_memory() as f64;
    let mem_used = sys.used_memory() as f64;
    let mem_ratio = if mem_total > 0.0 { mem_used / mem_total } else { 0.0 };
    let coherence = 1.0 - (avg_cpu / 100.0 - mem_ratio).abs(); // Closer to 1 = better alignment

    // Resonance Frequency: estimated in Hz based on task scheduling activity
    // Approximate: more stable systems have more regular task patterns
    let resonance = 50.0 + (stability * 30.0); // Range 50-80 Hz for stable systems

    (stability.min(1.0).max(0.0), coherence.min(1.0).max(0.0), resonance)
}

pub fn run(duration_secs: u64, interval_secs: u64) -> ConvergenceResult {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);
    let interval = Duration::from_secs(interval_secs);

    let mut points = Vec::new();
    let mut measurement_id = 0u32;

    while start.elapsed() < duration {
        let (stability, coherence, resonance) = measure_coherence();

        points.push(ConvergencePoint {
            timestamp: chrono::Utc::now().to_rfc3339(),
            measurement_id,
            stability_index: (stability * 100.0 * 10.0).round() / 10.0,
            coherence_level: (coherence * 100.0 * 10.0).round() / 10.0,
            resonance_frequency: (resonance * 10.0).round() / 10.0,
        });

        measurement_id += 1;

        if start.elapsed() < duration {
            std::thread::sleep(interval);
        }
    }

    // Analyze convergence: trend toward stable coherence
    let stability_avg = if !points.is_empty() {
        points.iter().map(|p| p.stability_index).sum::<f64>() / points.len() as f64
    } else {
        0.0
    };

    let coherence_avg = if !points.is_empty() {
        points.iter().map(|p| p.coherence_level).sum::<f64>() / points.len() as f64
    } else {
        0.0
    };

    // Convergence confidence: how consistent the measurements are
    let stability_variance = if !points.is_empty() {
        points.iter()
            .map(|p| (p.stability_index - stability_avg).powi(2))
            .sum::<f64>()
            / points.len() as f64
    } else {
        100.0
    };

    let convergence_confidence = 1.0 / (1.0 + stability_variance.sqrt());
    let converged = stability_avg > 50.0 && coherence_avg > 50.0 && convergence_confidence > 0.7;

    let system_status = match (stability_avg, coherence_avg) {
        (s, c) if s > 75.0 && c > 75.0 => "OPTIMAL_CONVERGENCE",
        (s, c) if s > 60.0 && c > 60.0 => "STABLE_CONVERGENCE",
        (s, c) if s > 40.0 && c > 40.0 => "PARTIAL_CONVERGENCE",
        _ => "DIVERGENT",
    };

    ConvergenceResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_measurements: points.len(),
        convergence_points: points,
        stability_average: (stability_avg * 10.0).round() / 10.0,
        coherence_average: (coherence_avg * 10.0).round() / 10.0,
        system_status: system_status.to_string(),
        converged,
        convergence_confidence: (convergence_confidence * 100.0 * 10.0).round() / 10.0,
    }
}

fn format_pretty(result: &ConvergenceResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  ◈ OMEGA-POINT \n"));
    out.push_str(&format!("║  Layer: Harmonic Resonance / Convergence Monitor\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ Convergence Analysis\n"));
    out.push_str(&format!("    Measurements: {}\n", result.total_measurements));
    out.push_str(&format!("    Duration: {} measurement points\n\n", result.total_measurements));

    out.push_str(&format!("  ▸ Aggregates\n"));
    out.push_str(&format!("    Stability Average:    {:.1}%\n", result.stability_average));
    out.push_str(&format!("    Coherence Average:    {:.1}%\n", result.coherence_average));
    out.push_str(&format!("    Convergence Confidence: {:.1}%\n\n", result.convergence_confidence));

    out.push_str(&format!("  ▸ Recent Points (last 5)\n"));
    let start_idx = result.convergence_points.len().saturating_sub(5);
    for point in &result.convergence_points[start_idx..] {
        out.push_str(&format!(
            "    [{}] S:{:.1}% C:{:.1}% F:{:.1}Hz\n",
            point.measurement_id,
            point.stability_index,
            point.coherence_level,
            point.resonance_frequency
        ));
    }

    out.push_str("\n");
    if result.converged {
        out.push_str(&format!("  ✓ System Status: {} (CONVERGED)\n", result.system_status));
    } else {
        out.push_str(&format!("  ✗ System Status: {} (NOT CONVERGED)\n", result.system_status));
    }

    out.push_str(&format!("\n  ⟫ omega-point :: {}\n\n", result.timestamp));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let result = run(cli.duration, cli.interval);

    if !cli.echo_x.is_empty() && cli.echo_x != "127.0.0.1:8888" {
        if let Ok(client) = bio_client::DroneClient::connect("omega-point", &cli.echo_x).await {
            let result_json = serde_json::json!({
                "status": "OK",
                "converged": result.converged,
                "stability_average": result.stability_average,
                "coherence_average": result.coherence_average,
                "convergence_confidence": result.convergence_confidence,
                "system_status": result.system_status,
            });
            let result_str = serde_json::to_string(&result_json).unwrap_or_default();
            let result_bytes = result_str.as_bytes();
            let _ = client.send_result(&[
                ("status", b"OK" as &[u8]),
                ("data", result_bytes),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
