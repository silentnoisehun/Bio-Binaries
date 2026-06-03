use crate::bio_client;
use clap::Parser;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "eqm-pulse", about = "System health monitor + FFT frequency analysis")]
pub struct Cli {
    /// Sampling duration in seconds
    #[arg(long, default_value = "5")]
    pub duration: u64,

    /// Sampling interval in milliseconds
    #[arg(long, default_value = "200")]
    pub interval: u64,


    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FrequencyComponent {
    pub frequency_hz: f64,
    pub amplitude: f64,
    pub phase_rad: f64,
}

#[derive(Debug, Serialize)]
pub struct SystemSnapshot {
    pub cpu_count: usize,
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Debug, Serialize)]
pub struct PulseResult {
    pub timestamp: String,
    pub snapshot: SystemSnapshot,
    pub cpu_samples: usize,
    pub fft_analysis: FftAnalysis,
    pub resonance_score: f64,
    pub health_status: String,
}

#[derive(Debug, Serialize)]
pub struct FftAnalysis {
    pub sample_rate_hz: f64,
    pub dominant_frequencies: Vec<FrequencyComponent>,
    pub spectral_energy: f64,
    pub stability_index: f64,
}

pub fn analyze_fft(samples: &[f32], sample_rate: f64) -> FftAnalysis {
    let n = samples.len().next_power_of_two();
    let mut input: Vec<Complex<f64>> = samples.iter()
        .map(|&s| Complex::new(s as f64, 0.0))
        .collect();
    input.resize(n, Complex::new(0.0, 0.0));

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    fft.process(&mut input);

    // Extract frequency components
    let freq_resolution = sample_rate / n as f64;
    let half_n = n / 2;
    let mut magnitudes: Vec<(f64, f64, f64)> = (1..half_n).map(|i| {
        let mag = (input[i].re.powi(2) + input[i].im.powi(2)).sqrt() / n as f64;
        let phase = input[i].im.atan2(input[i].re);
        let freq = i as f64 * freq_resolution;
        (freq, mag, phase)
    }).collect();

    magnitudes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let dominant: Vec<FrequencyComponent> = magnitudes.iter().take(5).map(|(f, a, p)| {
        FrequencyComponent {
            frequency_hz: (*f * 1000.0).round() / 1000.0,
            amplitude: (*a * 1000.0).round() / 1000.0,
            phase_rad: (*p * 1000.0).round() / 1000.0,
        }
    }).collect();

    let spectral_energy: f64 = magnitudes.iter().map(|(_, a, _)| a.powi(2)).sum();
    let max_amp = magnitudes.first().map(|(_, a, _)| *a).unwrap_or(0.0);
    let total_amp: f64 = magnitudes.iter().map(|(_, a, _)| *a).sum();
    let stability = if total_amp > 0.0 { 1.0 - (max_amp / total_amp) } else { 1.0 };

    FftAnalysis {
        sample_rate_hz: sample_rate,
        dominant_frequencies: dominant,
        spectral_energy: (spectral_energy * 1000.0).round() / 1000.0,
        stability_index: (stability * 1000.0).round() / 1000.0,
    }
}

pub fn run(duration: u64, interval: u64) -> PulseResult {
    // Capture system snapshot
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    let snapshot = SystemSnapshot {
        cpu_count: sys.cpus().len(),
        cpu_usage: sys.global_cpu_info().cpu_usage(),
        memory_used: sys.used_memory(),
        memory_total: sys.total_memory(),
    };

    // Collect CPU samples over duration
    let mut cpu_samples: Vec<f32> = Vec::new();
    let num_samples = (duration * 1000) / interval;
    let sample_rate = 1000.0 / interval as f64;

    {
        use sysinfo::{CpuRefreshKind, RefreshKind, System};
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        for _ in 0..num_samples {
            sys.refresh_cpu();
            cpu_samples.push(sys.global_cpu_info().cpu_usage());
            std::thread::sleep(std::time::Duration::from_millis(interval));
        }
    }

    let fft_analysis = analyze_fft(&cpu_samples, sample_rate);

    // Resonance score: combine stability with usage level
    let avg_cpu: f32 = if !cpu_samples.is_empty() {
        cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
    } else {
        0.0
    };
    let resonance = (fft_analysis.stability_index * 0.6 + (1.0 - avg_cpu as f64 / 100.0) * 0.4) * 100.0;

    let health = if resonance > 70.0 {
        "RESONANT"
    } else if resonance > 40.0 {
        "OSCILLATING"
    } else {
        "CHAOTIC"
    };

    PulseResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        snapshot,
        cpu_samples: cpu_samples.len(),
        fft_analysis,
        resonance_score: (resonance * 10.0).round() / 10.0,
        health_status: health.to_string(),
    }
}

fn format_pretty(result: &PulseResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  💓 EQM-PULSE \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / Health Monitor\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ System Snapshot\n"));
    out.push_str(&format!("    CPUs: {}\n", result.snapshot.cpu_count));
    out.push_str(&format!("    CPU Usage: {:.1}%\n", result.snapshot.cpu_usage));
    let mem_percent = if result.snapshot.memory_total > 0 {
        (result.snapshot.memory_used as f64 / result.snapshot.memory_total as f64) * 100.0
    } else {
        0.0
    };
    out.push_str(&format!("    Memory: {:.1}%\n", mem_percent));

    out.push_str("\n");
    out.push_str(&format!("  ▸ FFT Analysis ({} samples @ {:.1}Hz)\n",
        result.cpu_samples, result.fft_analysis.sample_rate_hz));
    for freq in &result.fft_analysis.dominant_frequencies {
        out.push_str(&format!("    {:.3} Hz: amp={:.3} phase={:.3}rad\n",
            freq.frequency_hz, freq.amplitude, freq.phase_rad));
    }
    out.push_str(&format!("    Spectral Energy: {:.3}\n", result.fft_analysis.spectral_energy));
    out.push_str(&format!("    Stability Index: {:.3}\n", result.fft_analysis.stability_index));

    out.push_str("\n");
    out.push_str(&format!("  ▸ Resonance\n"));
    out.push_str(&format!("    Score: {:.1}\n", result.resonance_score));
    out.push_str(&format!("    Health: {}\n", result.health_status));

    out.push_str(&format!("\n  ⟫ eqm-pulse :: {}\n\n", result.health_status));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(cli.duration, cli.interval);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("eqm-pulse", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
