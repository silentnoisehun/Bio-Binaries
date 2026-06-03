use crate::{bio_client, output};
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "wave-sculptor", about = "Frequency filter — digital signal processing on wave packets")]
pub struct Cli {
    /// Input wave packet JSON file (from wave-encoder)
    pub input: String,

    /// Filter type: lowpass, highpass, bandpass
    #[arg(long, default_value = "lowpass")]
    pub filter: String,

    /// Cutoff frequency (Hz) for lowpass/highpass
    #[arg(long, default_value = "1000.0")]
    pub cutoff: f64,

    /// Low bound for bandpass (Hz)
    #[arg(long, default_value = "100.0")]
    pub band_low: f64,

    /// High bound for bandpass (Hz)
    #[arg(long, default_value = "2000.0")]
    pub band_high: f64,

    /// Output file
    #[arg(long, short)]
    pub output: Option<String>,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FrequencyBin {
    pub frequency_hz: f64,
    pub amplitude: f64,
    pub phase_rad: f64,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize)]
pub struct InterferencePoint {
    pub frequency_hz: f64,
    pub original_amplitude: f64,
    pub filtered_amplitude: f64,
    pub interference_type: String, // constructive or destructive
}

#[derive(Debug, Serialize)]
pub struct SculptResult {
    pub timestamp: String,
    pub source: String,
    pub filter_type: String,
    pub filter_params: serde_json::Value,
    pub original_bins: usize,
    pub filtered_bins: usize,
    pub bins_removed: usize,
    pub filtered_packet: Vec<FrequencyBin>,
    pub interference_map: Vec<InterferencePoint>,
    pub energy_before: f64,
    pub energy_after: f64,
    pub energy_ratio: f64,
}

pub fn run(input: &str, filter: &str, cutoff: f64, band_low: f64, band_high: f64) -> Result<SculptResult, String> {
    let json = std::fs::read_to_string(input)
        .map_err(|e| format!("Cannot read input: {}", e))?;
    let packet: WavePacket = serde_json::from_str(&json)
        .map_err(|e| format!("Invalid wave packet JSON: {}", e))?;

    let original_count = packet.bins.len();
    let energy_before = packet.bins.iter().map(|b| b.amplitude.powi(2)).sum::<f64>();

    let mut interference_map = Vec::new();
    let mut filtered: Vec<FrequencyBin> = Vec::new();

    for bin in &packet.bins {
        let pass = match filter {
            "lowpass" => bin.frequency_hz <= cutoff,
            "highpass" => bin.frequency_hz >= cutoff,
            "bandpass" => bin.frequency_hz >= band_low && bin.frequency_hz <= band_high,
            _ => true,
        };

        let filtered_amp = if pass { bin.amplitude } else { 0.0 };
        let itype = if pass { "constructive" } else { "destructive" };

        interference_map.push(InterferencePoint {
            frequency_hz: bin.frequency_hz,
            original_amplitude: bin.amplitude,
            filtered_amplitude: filtered_amp,
            interference_type: itype.to_string(),
        });

        if pass {
            filtered.push(bin.clone());
        }
    }

    let energy_after = filtered.iter().map(|b| b.amplitude.powi(2)).sum::<f64>();
    let energy_ratio = if energy_before > 0.0 { energy_after / energy_before } else { 0.0 };

    let params = match filter {
        "lowpass" => serde_json::json!({"cutoff_hz": cutoff}),
        "highpass" => serde_json::json!({"cutoff_hz": cutoff}),
        "bandpass" => serde_json::json!({"low_hz": band_low, "high_hz": band_high}),
        _ => serde_json::json!({}),
    };

    Ok(SculptResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source: input.to_string(),
        filter_type: filter.to_string(),
        filter_params: params,
        original_bins: original_count,
        filtered_bins: filtered.len(),
        bins_removed: original_count - filtered.len(),
        filtered_packet: filtered,
        interference_map,
        energy_before: (energy_before * 10000.0).round() / 10000.0,
        energy_after: (energy_after * 10000.0).round() / 10000.0,
        energy_ratio: (energy_ratio * 10000.0).round() / 10000.0,
    })
}

fn print_pretty(result: &SculptResult) {
    output::banner("WAVE-SCULPTOR", "Resonance / Frequency Filter", "🎨");

    output::section("Filter Configuration");
    output::kv("Source", &result.source);
    output::kv("Filter", &result.filter_type);
    output::kv("Params", &result.filter_params.to_string());

    println!();
    output::section("Results");
    output::kv("Original Bins", &result.original_bins.to_string());
    output::kv("Filtered Bins", &result.filtered_bins.to_string());
    output::kv("Bins Removed", &result.bins_removed.to_string());
    output::kv("Energy Before", &format!("{:.4}", result.energy_before));
    output::kv("Energy After", &format!("{:.4}", result.energy_after));
    output::kv("Energy Ratio", &format!("{:.4}", result.energy_ratio));

    println!();
    output::section("Top Filtered Frequencies");
    for bin in result.filtered_packet.iter().take(10) {
        output::kv(
            &format!("{:.3} Hz", bin.frequency_hz),
            &format!("amp={:.5}", bin.amplitude),
        );
    }

    let constructive = result.interference_map.iter().filter(|i| i.interference_type == "constructive").count();
    let destructive = result.interference_map.iter().filter(|i| i.interference_type == "destructive").count();
    output::summary("wave-sculptor", &format!("{} constructive, {} destructive", constructive, destructive));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.input, &cli.filter, cli.cutoff, cli.band_low, cli.band_high)?;

    // Write filtered packet to file if requested
    if let Some(ref out_path) = cli.output {
        if let Ok(json) = serde_json::to_string_pretty(&result.filtered_packet) {
            std::fs::write(out_path, json).map_err(|e| e.to_string())?;
        }
    }

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("wave-sculptor", addr).await {
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
