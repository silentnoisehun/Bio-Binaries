use crate::{bio_client, output};
use clap::Parser;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "wave-encoder", about = "Data→wave encoder — FFT-based file encoding (432Hz base)")]
pub struct Cli {
    /// Input file to encode
    pub input: String,

    /// Base frequency (Hz)
    #[arg(long, default_value = "432.0")]
    pub base_freq: f64,

    /// Output wave packet file (JSON)
    #[arg(long, short)]
    pub output: Option<String>,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FrequencyBin {
    pub frequency_hz: f64,
    pub amplitude: f64,
    pub phase_rad: f64,
}

#[derive(Debug, Serialize)]
pub struct WavePacket {
    pub timestamp: String,
    pub source_file: String,
    pub source_size_bytes: usize,
    pub base_frequency_hz: f64,
    pub blake3_hash: String,
    pub bins: Vec<FrequencyBin>,
    pub total_spectral_energy: f64,
    pub dominant_frequency_hz: f64,
    pub encoding_fidelity: f64,
}

pub fn run(input: &str, base_freq: f64) -> WavePacket {
    let data = std::fs::read(input).unwrap_or_default();
    let hash = blake3::hash(&data).to_hex().to_string();
    let size = data.len();

    // Convert bytes to signal (normalize to -1.0..1.0)
    let signal: Vec<f64> = data.iter().map(|&b| (b as f64 - 128.0) / 128.0).collect();

    // Pad to next power of 2
    let n = signal.len().next_power_of_two().max(64);
    let mut input_complex: Vec<Complex<f64>> = signal.iter()
        .map(|&s| Complex::new(s, 0.0))
        .collect();
    input_complex.resize(n, Complex::new(0.0, 0.0));

    // FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    fft.process(&mut input_complex);

    // Extract frequency bins relative to base frequency
    let half_n = n / 2;
    let freq_resolution = base_freq / n as f64;

    let mut bins: Vec<FrequencyBin> = (1..half_n).map(|i| {
        let amplitude = (input_complex[i].re.powi(2) + input_complex[i].im.powi(2)).sqrt() / n as f64;
        let phase = input_complex[i].im.atan2(input_complex[i].re);
        let freq = i as f64 * freq_resolution * (base_freq / freq_resolution / n as f64 + 1.0);
        FrequencyBin {
            frequency_hz: (freq * 1000.0).round() / 1000.0,
            amplitude: (amplitude * 100000.0).round() / 100000.0,
            phase_rad: (phase * 10000.0).round() / 10000.0,
        }
    }).collect();

    // Sort by amplitude descending
    bins.sort_by(|a, b| b.amplitude.partial_cmp(&a.amplitude).unwrap_or(std::cmp::Ordering::Equal));

    let total_energy: f64 = bins.iter().map(|b| b.amplitude.powi(2)).sum();
    let dominant = bins.first().map(|b| b.frequency_hz).unwrap_or(0.0);

    // Encoding fidelity: how much energy is in top 10% of bins
    let top_10_pct = bins.len() / 10;
    let top_energy: f64 = bins.iter().take(top_10_pct.max(1)).map(|b| b.amplitude.powi(2)).sum();
    let fidelity = if total_energy > 0.0 { top_energy / total_energy } else { 0.0 };

    // Keep only significant bins (top 256)
    bins.truncate(256);

    WavePacket {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source_file: input.to_string(),
        source_size_bytes: size,
        base_frequency_hz: base_freq,
        blake3_hash: hash,
        bins,
        total_spectral_energy: (total_energy * 10000.0).round() / 10000.0,
        dominant_frequency_hz: dominant,
        encoding_fidelity: (fidelity * 10000.0).round() / 10000.0,
    }
}

fn print_pretty(result: &WavePacket) {
    output::banner("WAVE-ENCODER", "Resonance / FFT Data Encoder", "🌊");

    output::section("Source");
    output::kv("File", &result.source_file);
    output::kv("Size", &format!("{} bytes", result.source_size_bytes));
    output::kv("BLAKE3", &result.blake3_hash[..32]);
    output::kv("Base Frequency", &format!("{} Hz", result.base_frequency_hz));

    println!();
    output::section("Wave Packet");
    output::kv("Frequency Bins", &result.bins.len().to_string());
    output::kv("Spectral Energy", &format!("{:.4}", result.total_spectral_energy));
    output::kv("Dominant Frequency", &format!("{:.3} Hz", result.dominant_frequency_hz));
    output::kv("Encoding Fidelity", &format!("{:.4}", result.encoding_fidelity));

    println!();
    output::section("Top Frequencies");
    for bin in result.bins.iter().take(10) {
        output::kv(
            &format!("{:.3} Hz", bin.frequency_hz),
            &format!("amp={:.5} phase={:.4}rad", bin.amplitude, bin.phase_rad),
        );
    }

    output::summary("wave-encoder", &format!("{} bins encoded", result.bins.len()));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.input, cli.base_freq);

    // Write wave packet to file if requested
    if let Some(ref out_path) = cli.output {
        if let Ok(json) = serde_json::to_string_pretty(&result) {
            std::fs::write(out_path, json).map_err(|e| e.to_string())?;
        }
    }

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("wave-encoder", addr).await {
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
