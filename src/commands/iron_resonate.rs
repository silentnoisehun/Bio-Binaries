use crate::{bio_client, output};
use clap::Parser;
use serde::Serialize;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

#[derive(Parser)]
#[command(name = "iron-resonate", about = "Hardware resonance monitor — detailed HW performance profiler")]
pub struct Cli {
    /// Number of samples for stability measurement
    #[arg(long, default_value = "10")]
    pub samples: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CoreResonance {
    pub core_id: usize,
    pub frequency_mhz: u64,
    pub usage_percent: f32,
    pub amplitude: f64,
    pub stability: f64,
}

#[derive(Debug, Serialize)]
pub struct MemResonance {
    pub total_mb: u64,
    pub used_mb: u64,
    pub usage_percent: f64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
}

#[derive(Debug, Serialize)]
pub struct DiskResonance {
    pub name: String,
    pub mount_point: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub usage_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct ResonateResult {
    pub timestamp: String,
    pub cores: Vec<CoreResonance>,
    pub memory: MemResonance,
    pub disks: Vec<DiskResonance>,
    pub global_resonance: f64,
    pub stability_rating: String,
}

pub fn run(samples: usize) -> ResonateResult {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    // Collect multiple samples for stability measurement
    let mut cpu_history: Vec<Vec<f32>> = Vec::new();
    for _ in 0..samples {
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu();
        let sample: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        cpu_history.push(sample);
    }
    sys.refresh_memory();

    let num_cores = sys.cpus().len();
    let mut cores = Vec::new();

    for i in 0..num_cores {
        let cpu = &sys.cpus()[i];
        let values: Vec<f32> = cpu_history.iter().map(|s| s.get(i).copied().unwrap_or(0.0)).collect();
        let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;
        let variance: f32 = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
        let std_dev = variance.sqrt();
        let stability = if mean > 0.0 { 1.0 - (std_dev as f64 / mean as f64).min(1.0) } else { 1.0 };

        cores.push(CoreResonance {
            core_id: i,
            frequency_mhz: cpu.frequency(),
            usage_percent: cpu.cpu_usage(),
            amplitude: (cpu.cpu_usage() as f64 / 100.0 * 1000.0).round() / 1000.0,
            stability: (stability * 1000.0).round() / 1000.0,
        });
    }

    let memory = MemResonance {
        total_mb: sys.total_memory() / (1024 * 1024),
        used_mb: sys.used_memory() / (1024 * 1024),
        usage_percent: (sys.used_memory() as f64 / sys.total_memory().max(1) as f64 * 1000.0).round() / 10.0,
        swap_total_mb: sys.total_swap() / (1024 * 1024),
        swap_used_mb: sys.used_swap() / (1024 * 1024),
    };

    let disk_info = Disks::new_with_refreshed_list();
    let disks: Vec<DiskResonance> = disk_info.iter().map(|d| {
        let total = d.total_space() as f64 / 1_073_741_824.0;
        let avail = d.available_space() as f64 / 1_073_741_824.0;
        DiskResonance {
            name: d.name().to_string_lossy().to_string(),
            mount_point: d.mount_point().to_string_lossy().to_string(),
            total_gb: (total * 100.0).round() / 100.0,
            available_gb: (avail * 100.0).round() / 100.0,
            usage_percent: if total > 0.0 { ((total - avail) / total * 1000.0).round() / 10.0 } else { 0.0 },
        }
    }).collect();

    let avg_stability: f64 = cores.iter().map(|c| c.stability).sum::<f64>() / cores.len().max(1) as f64;
    let avg_usage: f64 = cores.iter().map(|c| c.usage_percent as f64).sum::<f64>() / cores.len().max(1) as f64;
    let global_resonance = (avg_stability * 0.6 + (1.0 - avg_usage / 100.0) * 0.4) * 100.0;

    let rating = if global_resonance > 80.0 { "HARMONIC" }
        else if global_resonance > 50.0 { "RESONANT" }
        else if global_resonance > 30.0 { "DISSONANT" }
        else { "CHAOTIC" };

    ResonateResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        cores,
        memory,
        disks,
        global_resonance: (global_resonance * 10.0).round() / 10.0,
        stability_rating: rating.to_string(),
    }
}

fn print_pretty(result: &ResonateResult) {
    output::banner("IRON-RESONATE", "Resonance / HW Performance Profiler", "🔩");

    output::section("CPU Cores");
    for c in &result.cores {
        output::kv(
            &format!("Core {}", c.core_id),
            &format!("{}MHz  usage={:.1}%  amp={:.3}  stab={:.3}", c.frequency_mhz, c.usage_percent, c.amplitude, c.stability),
        );
    }

    println!();
    output::section("Memory");
    output::progress_bar("RAM", result.memory.usage_percent, 100.0);
    output::kv("Detail", &format!("{}/{}MB", result.memory.used_mb, result.memory.total_mb));
    if result.memory.swap_total_mb > 0 {
        output::kv("Swap", &format!("{}/{}MB", result.memory.swap_used_mb, result.memory.swap_total_mb));
    }

    println!();
    output::section("Disks");
    for d in &result.disks {
        output::progress_bar(&d.mount_point, d.usage_percent, 100.0);
    }

    println!();
    output::status("Global Resonance", result.global_resonance, "");
    output::summary("iron-resonate", &result.stability_rating);
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(cli.samples);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("iron-resonate", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    print_pretty(&result);
    Ok(String::new())
}
