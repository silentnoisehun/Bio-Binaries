use crate::{acoustic, bio_client, magneto};
use clap::Parser;
use serde::Serialize;
use std::f64::consts::PI;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "magneto-acoustic", about = "Code health sonifier — error patterns to audio")]
pub struct Cli {
    /// Project directory to scan
    pub dir: String,

    /// Output WAV file path
    #[arg(long)]
    pub output: Option<String>,

    /// Tone duration in milliseconds
    #[arg(long, default_value = "100")]
    pub tone_ms: u32,

    /// Analysis depth (0=shallow, 3=deep)
    #[arg(long, default_value = "2")]
    pub depth: u32,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AcousticOutput {
    pub status: String,
    pub wav_path: String,
    pub duration_secs: f64,
    pub sample_count: usize,
    pub files_scanned: usize,
    pub total_hotspots: usize,
    pub tension_score: f64,
    pub drone_freq_hz: f64,
}

const SAMPLE_RATE: u32 = 8000;
const AMPLITUDE: f64 = 0.5;
const FADE_MS: u32 = 10;

/// Map pattern name to frequency
fn pattern_freq(pattern: &str) -> f64 {
    match pattern {
        "PANIC"   => 200.0,
        "ERROR"   => 400.0,
        "BUG"     => 400.0,
        "HACK"    => 600.0,
        "FIXME"   => 700.0,
        "UNSAFE"  => 800.0,
        "WARNING" => 1000.0,
        "DONE"    => 1200.0,
        "UNWRAP"  => 1400.0,
        _         => 900.0,
    }
}

/// Generate a sine tone burst with fade-in/out
fn tone_burst(freq: f64, duration_ms: u32, sample_rate: u32, amplitude: f64) -> Vec<i16> {
    let total_samples = (sample_rate as f64 * duration_ms as f64 / 1000.0) as usize;
    let fade_samples = (sample_rate as f64 * FADE_MS as f64 / 1000.0) as usize;
    let max_val = (i16::MAX as f64) * amplitude;

    (0..total_samples)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            let sample = (2.0 * PI * freq * t).sin();

            // Fade envelope
            let envelope = if i < fade_samples {
                i as f64 / fade_samples as f64
            } else if i > total_samples - fade_samples {
                (total_samples - i) as f64 / fade_samples as f64
            } else {
                1.0
            };

            (sample * envelope * max_val) as i16
        })
        .collect()
}

/// Generate background drone tone
fn drone_tone(tension: f64, duration_samples: usize, sample_rate: u32) -> Vec<i16> {
    let freq = 80.0 + 20.0 / (tension + 0.1);
    let max_val = (i16::MAX as f64) * 0.15; // Quiet background

    (0..duration_samples)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            ((2.0 * PI * freq * t).sin() * max_val) as i16
        })
        .collect()
}

/// Generate A-major harmony chord (440/554/659 Hz)
fn harmony_chord(duration_ms: u32, sample_rate: u32) -> Vec<i16> {
    let total = (sample_rate as f64 * duration_ms as f64 / 1000.0) as usize;
    let fade_samples = (sample_rate as f64 * FADE_MS as f64 / 1000.0) as usize;
    let freqs = [440.0, 554.0, 659.0];
    let amp = (i16::MAX as f64) * 0.25;

    (0..total)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            let sum: f64 = freqs.iter().map(|f| (2.0 * PI * f * t).sin()).sum();
            let envelope = if i < fade_samples {
                i as f64 / fade_samples as f64
            } else if i > total - fade_samples {
                (total - i) as f64 / fade_samples as f64
            } else {
                1.0
            };
            (sum / freqs.len() as f64 * envelope * amp) as i16
        })
        .collect()
}

pub fn run(dir: &str, output_path: &str, tone_ms: u32, depth: usize) -> AcousticOutput {
    let geo = magneto::run(dir, depth);

    // Compute total audio duration
    // Each hotspot gets a tone burst; clean code gets a chord
    let tone_samples = (SAMPLE_RATE as f64 * tone_ms as f64 / 1000.0) as usize;
    let gap_samples = (SAMPLE_RATE as f64 * 0.05) as usize; // 50ms gap between tones

    let has_hotspots = !geo.hotspots.is_empty();
    let num_tones = if has_hotspots { geo.hotspots.len().min(50) } else { 1 }; // cap at 50
    let total_samples = num_tones * (tone_samples + gap_samples) + gap_samples;

    // Generate drone background
    let drone_freq = 80.0 + 20.0 / (geo.tension_score + 0.1);
    let mut samples = drone_tone(geo.tension_score, total_samples, SAMPLE_RATE);

    if has_hotspots {
        // Overlay tone bursts for each hotspot
        let mut offset = gap_samples;
        for hotspot in geo.hotspots.iter().take(50) {
            let freq = pattern_freq(&hotspot.pattern);
            let burst = tone_burst(freq, tone_ms, SAMPLE_RATE, AMPLITUDE);

            for (i, &s) in burst.iter().enumerate() {
                let idx = offset + i;
                if idx < samples.len() {
                    samples[idx] = samples[idx].saturating_add(s);
                }
            }
            offset += tone_samples + gap_samples;
        }
    } else {
        // Clean code: A-major harmony
        let chord = harmony_chord(tone_ms * 3, SAMPLE_RATE);
        let start = gap_samples;
        for (i, &s) in chord.iter().enumerate() {
            let idx = start + i;
            if idx < samples.len() {
                samples[idx] = samples[idx].saturating_add(s);
            }
        }
    }

    // Write WAV
    let wav_path = PathBuf::from(output_path);
    let duration_secs = samples.len() as f64 / SAMPLE_RATE as f64;

    match acoustic::write_wav(&wav_path, &samples, SAMPLE_RATE) {
        Ok(()) => AcousticOutput {
            status: "OK".into(),
            wav_path: wav_path.to_string_lossy().to_string(),
            duration_secs: (duration_secs * 100.0).round() / 100.0,
            sample_count: samples.len(),
            files_scanned: geo.files_scanned,
            total_hotspots: geo.total_hotspots,
            tension_score: geo.tension_score,
            drone_freq_hz: (drone_freq * 10.0).round() / 10.0,
        },
        Err(e) => AcousticOutput {
            status: format!("ERROR: {}", e),
            wav_path: String::new(),
            duration_secs: 0.0,
            sample_count: 0,
            files_scanned: geo.files_scanned,
            total_hotspots: geo.total_hotspots,
            tension_score: geo.tension_score,
            drone_freq_hz: drone_freq,
        },
    }
}

fn format_pretty(result: &AcousticOutput) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "🔊", "MAGNETO-ACOUSTIC"));
    out.push_str(&format!("║  Layer: {}\n", "Code Health Sonifier"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str("  ▸ Scan Results\n");
    out.push_str(&format!("    Status: {}\n", result.status));
    out.push_str(&format!("    Files Scanned: {}\n", result.files_scanned));
    out.push_str(&format!("    Total Hotspots: {}\n", result.total_hotspots));
    out.push_str(&format!("    Tension Score: {:.2}\n", result.tension_score));

    out.push('\n');
    out.push_str("  ▸ Audio Output\n");
    out.push_str(&format!("    WAV Path: {}\n", result.wav_path));
    out.push_str(&format!("    Duration: {:.2}s\n", result.duration_secs));
    out.push_str(&format!("    Samples: {}\n", result.sample_count));
    out.push_str(&format!("    Drone Freq: {:.1} Hz\n", result.drone_freq_hz));

    out.push_str(&format!(
        "\n  ⟫ magneto-acoustic :: {} hotspots → {:.2}s WAV\n\n",
        result.total_hotspots, result.duration_secs
    ));
    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let output_path = cli.output.as_deref().unwrap_or("output.wav");
    let result = run(&cli.dir, output_path, cli.tone_ms, cli.depth as usize);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("magneto-acoustic", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
