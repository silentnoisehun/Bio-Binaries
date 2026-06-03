/// Cryostasis Engine — Spectral snapshot system
///
/// Saves system state as FFT frequency-domain data ("CryoFrame").
/// Each frame captures CPU cores, spectral bands (hw/net/task),
/// resonance score, and stability index.
/// Frames are BLAKE3 integrity-checked and can be compressed
/// to fit in a single QR code (~600 bytes).

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

// ── Constants ──

pub const CRYO_VERSION: u8 = 1;
pub const CRYO_DIR_NAME: &str = ".bio-cryo";

/// Spectral band definitions (CT-analogy)
pub const BAND_HW: &str = "hw";       // 0-100 Hz: hardware state
pub const BAND_NET: &str = "net";      // 100-1000 Hz: network topology
pub const BAND_TASK: &str = "task";    // 1000+ Hz: running tasks, code integrity

// ── CryoFrame Flags ──
pub mod cryo_flags {
    pub const MANUAL_FREEZE: u8     = 0b0000_0001; // Manual trigger
    pub const SYSTEM_FREEZE: u8     = 0b0000_0010; // Queen system-wide freeze
    pub const DRIFT_WARNING: u8     = 0b0000_0100; // Resonance drift detected on thaw
    pub const INTEGRITY_OK: u8      = 0b0000_1000; // Binary integrity verified at freeze
    pub const SPECTRAL_ANOMALY: u8  = 0b0001_0000; // Spectral anomaly (dominant freq spike)
    pub const WAVE_MEMORY: u8       = 0b0010_0000; // Contains wave memory data
    pub const COMPRESSED: u8        = 0b0100_0000; // Data was compressed
    pub const MULTI_QR: u8          = 0b1000_0000; // Spans multiple QR codes

    pub fn names(flags: u8) -> Vec<&'static str> {
        let mut result = Vec::new();
        if flags & MANUAL_FREEZE != 0 { result.push("MANUAL"); }
        if flags & SYSTEM_FREEZE != 0 { result.push("SYSTEM"); }
        if flags & DRIFT_WARNING != 0 { result.push("DRIFT"); }
        if flags & INTEGRITY_OK != 0 { result.push("INTACT"); }
        if flags & SPECTRAL_ANOMALY != 0 { result.push("ANOMALY"); }
        if flags & WAVE_MEMORY != 0 { result.push("WAVE_MEM"); }
        if flags & COMPRESSED != 0 { result.push("ZLIB"); }
        if flags & MULTI_QR != 0 { result.push("MULTI_QR"); }
        result
    }
}

/// Wave memory entry — a dominant frequency captured during freeze
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveMemory {
    pub frequency_hz: f64,
    pub amplitude: f64,
    pub phase_rad: f64,
    pub band: String,
    pub core_id: Option<u16>,
}

// ── Error type ──

#[derive(Debug)]
pub enum CryoError {
    InvalidVersion(u8),
    DecompressionFailed(String),
    HashMismatch { expected: String, computed: String },
    IoError(std::io::Error),
    FrameCorrupted(String),
}

impl std::fmt::Display for CryoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVersion(v) => write!(f, "invalid cryo version: {}", v),
            Self::DecompressionFailed(e) => write!(f, "decompression failed: {}", e),
            Self::HashMismatch { expected, computed } =>
                write!(f, "hash mismatch: expected={} computed={}", expected, computed),
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::FrameCorrupted(msg) => write!(f, "frame corrupted: {}", msg),
        }
    }
}

impl From<std::io::Error> for CryoError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

// ── Spectral data structures ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralSlice {
    pub band_id: String,
    pub freq_low_hz: f64,
    pub freq_high_hz: f64,
    pub bin_count: usize,
    /// Interleaved amplitude + phase pairs: [amp0, phase0, amp1, phase1, ...]
    pub data: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreFreeze {
    pub core_id: u16,
    pub frequency_mhz: u16,
    pub usage: u16,       // usage * 10 (e.g., 45.3% → 453)
    pub amplitude: u16,   // primary FFT amplitude * 1000
    pub stability: u16,   // stability * 1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryoFrame {
    // Header
    pub version: u8,
    pub flags: u8,
    pub frozen_at: String,
    pub hostname: String,
    pub origin_binary: String,
    pub generation: u32,
    pub binary_hash: String,

    // System state
    pub cpu_global_x10: u16,
    pub memory_used_mb: u32,
    pub memory_total_mb: u32,
    pub process_count: u16,

    // Spectral data
    pub cores: Vec<CoreFreeze>,
    pub spectral_slices: Vec<SpectralSlice>,
    pub resonance_score_x10: u16,
    pub stability_index_x1000: u16,

    // Wave memory — dominant frequencies from each band + core
    pub wave_memory: Vec<WaveMemory>,

    // Drone registry
    pub drone_names: Vec<String>,

    // Integrity
    pub frame_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThawReport {
    pub frame_valid: bool,
    pub binary_match: bool,
    pub resonance_drift: f64,
    pub spectral_correlation: f64,
    pub thaw_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryoSaveResult {
    pub json_path: String,
    pub binary_path: String,
    pub compressed_size: usize,
    pub frame_hash: String,
}

// ── Spectral capture ──

/// Sample CPU usage over time and perform FFT to extract spectral data
pub fn capture_spectral(
    duration_ms: u64,
    interval_ms: u64,
    max_bins_per_band: usize,
) -> (Vec<CoreFreeze>, Vec<SpectralSlice>, u16, u16) {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu();

    let num_cores = sys.cpus().len();
    let num_samples = ((duration_ms / interval_ms) as usize).max(4);
    let sample_rate = 1000.0 / interval_ms as f64;

    // Collect CPU samples per core
    let mut core_samples: Vec<Vec<f32>> = vec![Vec::with_capacity(num_samples); num_cores];
    let mut global_samples: Vec<f32> = Vec::with_capacity(num_samples);

    for _ in 0..num_samples {
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
        sys.refresh_cpu();
        for (i, cpu) in sys.cpus().iter().enumerate() {
            if i < core_samples.len() {
                core_samples[i].push(cpu.cpu_usage());
            }
        }
        global_samples.push(sys.global_cpu_info().cpu_usage());
    }

    // FFT analysis per core → CoreFreeze
    let mut cores = Vec::new();
    let mut all_amplitudes: Vec<f64> = Vec::new();
    let mut all_stabilities: Vec<f64> = Vec::new();

    for (i, samples) in core_samples.iter().enumerate() {
        let (amp, stability) = fft_dominant(samples, sample_rate);
        let cpu = sys.cpus().get(i);
        cores.push(CoreFreeze {
            core_id: i as u16,
            frequency_mhz: cpu.map(|c| c.frequency() as u16).unwrap_or(0),
            usage: cpu.map(|c| (c.cpu_usage() * 10.0) as u16).unwrap_or(0),
            amplitude: (amp * 1000.0).min(65535.0) as u16,
            stability: (stability * 1000.0).min(65535.0) as u16,
        });
        all_amplitudes.push(amp);
        all_stabilities.push(stability);
    }

    // Build spectral slices from global samples
    let slices = build_spectral_slices(&global_samples, sample_rate, max_bins_per_band);

    // Resonance score: stability (60%) + idle CPU (40%)
    let avg_stability = if all_stabilities.is_empty() {
        1.0
    } else {
        all_stabilities.iter().sum::<f64>() / all_stabilities.len() as f64
    };
    let avg_cpu = if global_samples.is_empty() {
        0.0
    } else {
        global_samples.iter().map(|x| *x as f64).sum::<f64>() / global_samples.len() as f64
    };
    let idle_factor = (100.0 - avg_cpu) / 100.0;
    let resonance = avg_stability * 0.6 + idle_factor * 0.4;
    let resonance_x10 = (resonance * 1000.0).min(65535.0) as u16;
    let stability_x1000 = (avg_stability * 1000.0).min(65535.0) as u16;

    (cores, slices, resonance_x10, stability_x1000)
}

/// Run FFT on samples and return (dominant_amplitude, stability_index)
fn fft_dominant(samples: &[f32], _sample_rate: f64) -> (f64, f64) {
    if samples.len() < 2 {
        return (0.0, 1.0);
    }

    let n = samples.len().next_power_of_two();
    let mut input: Vec<Complex<f64>> = samples
        .iter()
        .map(|&s| Complex::new(s as f64, 0.0))
        .collect();
    input.resize(n, Complex::new(0.0, 0.0));

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    fft.process(&mut input);

    let half_n = n / 2;
    let mut magnitudes: Vec<f64> = (1..half_n)
        .map(|i| {
            (input[i].re.powi(2) + input[i].im.powi(2)).sqrt() / n as f64
        })
        .collect();
    magnitudes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let max_amp = magnitudes.first().copied().unwrap_or(0.0);
    let total_amp: f64 = magnitudes.iter().sum();
    let stability = if total_amp > 0.0 {
        1.0 - (max_amp / total_amp)
    } else {
        1.0
    };

    (max_amp, stability)
}

/// Build 3 spectral slices (hw, net, task) from samples
fn build_spectral_slices(
    samples: &[f32],
    sample_rate: f64,
    max_bins: usize,
) -> Vec<SpectralSlice> {
    if samples.len() < 2 {
        return vec![
            empty_slice(BAND_HW, 0.0, 100.0),
            empty_slice(BAND_NET, 100.0, 1000.0),
            empty_slice(BAND_TASK, 1000.0, 5000.0),
        ];
    }

    let n = samples.len().next_power_of_two();
    let mut input: Vec<Complex<f64>> = samples
        .iter()
        .map(|&s| Complex::new(s as f64, 0.0))
        .collect();
    input.resize(n, Complex::new(0.0, 0.0));

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    fft.process(&mut input);

    let freq_resolution = sample_rate / n as f64;
    let half_n = n / 2;

    // Extract frequency bins with (freq, amplitude, phase)
    let bins: Vec<(f64, f32, f32)> = (1..half_n)
        .map(|i| {
            let amp = (input[i].re.powi(2) + input[i].im.powi(2)).sqrt() / n as f64;
            let phase = input[i].im.atan2(input[i].re);
            let freq = i as f64 * freq_resolution;
            (freq, amp as f32, phase as f32)
        })
        .collect();

    // Split into bands and build slices
    let bands: Vec<(&str, f64, f64)> = vec![
        (BAND_HW, 0.0, 100.0),
        (BAND_NET, 100.0, 1000.0),
        (BAND_TASK, 1000.0, 5000.0),
    ];

    bands
        .into_iter()
        .map(|(name, low, high)| {
            let band_bins: Vec<&(f64, f32, f32)> = bins
                .iter()
                .filter(|(f, _, _)| *f >= low && *f < high)
                .collect();

            // Take up to max_bins, evenly distributed
            let selected = if band_bins.len() <= max_bins {
                band_bins
            } else {
                let step = band_bins.len() as f64 / max_bins as f64;
                (0..max_bins)
                    .map(|i| band_bins[(i as f64 * step) as usize])
                    .collect()
            };

            let bin_count = selected.len();
            let mut data = Vec::with_capacity(bin_count * 2);
            for &&(_, amp, phase) in &selected {
                data.push(amp);
                data.push(phase);
            }

            SpectralSlice {
                band_id: name.to_string(),
                freq_low_hz: low,
                freq_high_hz: high,
                bin_count,
                data,
            }
        })
        .collect()
}

fn empty_slice(band: &str, low: f64, high: f64) -> SpectralSlice {
    SpectralSlice {
        band_id: band.to_string(),
        freq_low_hz: low,
        freq_high_hz: high,
        bin_count: 0,
        data: vec![],
    }
}

// ── Freeze / Thaw ──

/// Capture a complete CryoFrame
pub fn freeze(
    generation: u32,
    drone_names: Vec<String>,
    duration_ms: u64,
    interval_ms: u64,
) -> CryoFrame {
    let (cores, slices, resonance_x10, stability_x1000) =
        capture_spectral(duration_ms, interval_ms, 32);

    // System snapshot
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(ProcessRefreshKind::everything()),
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu();
    sys.refresh_memory();
    sys.refresh_processes();

    let cpu_global_x10 = (sys.global_cpu_info().cpu_usage() * 10.0) as u16;
    let memory_used_mb = (sys.used_memory() / (1024 * 1024)) as u32;
    let memory_total_mb = (sys.total_memory() / (1024 * 1024)) as u32;
    let process_count = sys.processes().len().min(u16::MAX as usize) as u16;
    let hostname = System::host_name().unwrap_or_else(|| "unknown".into());

    // Binary hash
    let binary_hash = crate::auth::BinaryIntegrity::self_hash()
        .unwrap_or_else(|| "unknown".into());

    let origin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".into());

    // Build wave memory from spectral data
    let mut wave_mem = extract_wave_memory(&slices, &cores);

    // Append emotional wave bands from system state
    let emoti_waves = capture_emoti_waves(&sys);
    wave_mem.extend(emoti_waves);

    // Compute flags
    let mut flags: u8 = cryo_flags::WAVE_MEMORY | cryo_flags::COMPRESSED;
    if binary_hash != "unknown" {
        flags |= cryo_flags::INTEGRITY_OK;
    }
    // Check for spectral anomaly: any core amplitude > 800 (0.8 normalized)
    if cores.iter().any(|c| c.amplitude > 800) {
        flags |= cryo_flags::SPECTRAL_ANOMALY;
    }
    if !drone_names.is_empty() {
        flags |= cryo_flags::SYSTEM_FREEZE;
    } else {
        flags |= cryo_flags::MANUAL_FREEZE;
    }

    let mut frame = CryoFrame {
        version: CRYO_VERSION,
        flags,
        frozen_at: chrono::Utc::now().to_rfc3339(),
        hostname,
        origin_binary: origin,
        generation,
        binary_hash,
        cpu_global_x10,
        memory_used_mb,
        memory_total_mb,
        process_count,
        cores,
        spectral_slices: slices,
        resonance_score_x10: resonance_x10,
        stability_index_x1000: stability_x1000,
        wave_memory: wave_mem,
        drone_names,
        frame_hash: String::new(),
    };

    // Compute frame hash
    frame.frame_hash = compute_frame_hash(&frame);
    frame
}

/// Verify a CryoFrame's integrity
pub fn verify_frame(frame: &CryoFrame) -> bool {
    let computed = compute_frame_hash_without_hash(frame);
    computed == frame.frame_hash
}

/// Thaw: compare a frozen frame against current system state
pub fn thaw(frame: &CryoFrame) -> ThawReport {
    let frame_valid = verify_frame(frame);

    // Check binary hash
    let current_hash = crate::auth::BinaryIntegrity::self_hash()
        .unwrap_or_else(|| "unknown".into());
    let binary_match = frame.binary_hash == current_hash;

    // Capture current spectral for comparison
    let (_, current_slices, current_resonance_x10, _) =
        capture_spectral(500, 100, 32);

    // Resonance drift
    let frozen_resonance = frame.resonance_score_x10 as f64 / 10.0;
    let current_resonance = current_resonance_x10 as f64 / 10.0;
    let resonance_drift = (current_resonance - frozen_resonance).abs();

    // Spectral correlation (dot product of amplitude bins)
    let spectral_correlation = compute_spectral_correlation(
        &frame.spectral_slices,
        &current_slices,
    );

    let thaw_status = if !frame_valid {
        "CORRUPTED"
    } else if !binary_match {
        "BINARY_CHANGED"
    } else if resonance_drift > 50.0 {
        "DRIFT_HIGH"
    } else if spectral_correlation < 0.3 {
        "SPECTRAL_DIVERGENCE"
    } else {
        "OK"
    };

    ThawReport {
        frame_valid,
        binary_match,
        resonance_drift: (resonance_drift * 100.0).round() / 100.0,
        spectral_correlation: (spectral_correlation * 1000.0).round() / 1000.0,
        thaw_status: thaw_status.to_string(),
    }
}

fn compute_spectral_correlation(a: &[SpectralSlice], b: &[SpectralSlice]) -> f64 {
    let mut dot = 0.0_f64;
    let mut mag_a = 0.0_f64;
    let mut mag_b = 0.0_f64;

    for (sa, sb) in a.iter().zip(b.iter()) {
        // Compare amplitudes only (even indices in interleaved data)
        let amps_a: Vec<f32> = sa.data.iter().step_by(2).copied().collect();
        let amps_b: Vec<f32> = sb.data.iter().step_by(2).copied().collect();
        let len = amps_a.len().min(amps_b.len());
        for i in 0..len {
            dot += amps_a[i] as f64 * amps_b[i] as f64;
            mag_a += (amps_a[i] as f64).powi(2);
            mag_b += (amps_b[i] as f64).powi(2);
        }
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom > 0.0 { dot / denom } else { 0.0 }
}

// ── Binary encoding (using bincode) ──

/// Encode a CryoFrame into compact binary format
pub fn encode_binary(frame: &CryoFrame) -> Vec<u8> {
    bincode::serialize(frame).unwrap_or_default()
}

/// Decode a CryoFrame from binary format
pub fn decode_binary(data: &[u8]) -> Result<CryoFrame, CryoError> {
    bincode::deserialize(data)
        .map_err(|e| CryoError::FrameCorrupted(e.to_string()))
}

// ── Compression ──

/// Compress data with zlib
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    let _ = encoder.write_all(data);
    encoder.finish().unwrap_or_default()
}

/// Decompress zlib data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, CryoError> {
    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| CryoError::DecompressionFailed(e.to_string()))?;
    Ok(output)
}

// ── Hashing ──

/// Extract dominant frequencies from spectral slices + per-core data as wave memory
fn extract_wave_memory(slices: &[SpectralSlice], cores: &[CoreFreeze]) -> Vec<WaveMemory> {
    let mut waves = Vec::new();

    // Top frequencies from each spectral band
    for slice in slices {
        let mut best_amp: f64 = 0.0;
        let mut best_phase: f64 = 0.0;
        let mut best_freq: f64 = 0.0;

        let bin_count = slice.bin_count;
        if bin_count == 0 { continue; }
        let freq_step = (slice.freq_high_hz - slice.freq_low_hz) / bin_count as f64;

        for i in 0..bin_count {
            let amp = slice.data.get(i * 2).copied().unwrap_or(0.0) as f64;
            let phase = slice.data.get(i * 2 + 1).copied().unwrap_or(0.0) as f64;
            if amp > best_amp {
                best_amp = amp;
                best_phase = phase;
                best_freq = slice.freq_low_hz + i as f64 * freq_step;
            }
        }

        if best_amp > 0.0 {
            waves.push(WaveMemory {
                frequency_hz: (best_freq * 100.0).round() / 100.0,
                amplitude: (best_amp * 10000.0).round() / 10000.0,
                phase_rad: (best_phase * 1000.0).round() / 1000.0,
                band: slice.band_id.clone(),
                core_id: None,
            });
        }
    }

    // Per-core dominant (top 4 cores by amplitude)
    let mut sorted_cores: Vec<&CoreFreeze> = cores.iter().collect();
    sorted_cores.sort_by(|a, b| b.amplitude.cmp(&a.amplitude));
    for core in sorted_cores.iter().take(4) {
        if core.amplitude > 0 {
            waves.push(WaveMemory {
                frequency_hz: core.frequency_mhz as f64,
                amplitude: core.amplitude as f64 / 1000.0,
                phase_rad: 0.0,
                band: "core".to_string(),
                core_id: Some(core.core_id),
            });
        }
    }

    waves
}

/// Capture behavioral-metric wave bands from system state, mapped to emotional metaphors.
///
/// These are NOT emotion detection — they are system behavioral metrics renamed
/// for conceptual alignment with the bio-binary metaphor:
/// - `emoti:trust`     = CPU stability (low variance across cores → high "trust")
/// - `emoti:joy`       = idle CPU ratio (system breathing room → "joy")
/// - `emoti:curiosity` = process name variety (diverse workload → "curiosity")
fn capture_emoti_waves(sys: &System) -> Vec<WaveMemory> {
    let cpus = sys.cpus();
    let cpu_count = cpus.len().max(1) as f64;

    // Stability: average stability across cores (low variance = high stability)
    let usages: Vec<f64> = cpus.iter().map(|c| c.cpu_usage() as f64).collect();
    let avg_usage = usages.iter().sum::<f64>() / cpu_count;
    let variance = usages.iter().map(|u| (u - avg_usage).powi(2)).sum::<f64>() / cpu_count;
    let stability = (1.0 - (variance / 2500.0).min(1.0)).max(0.0); // 0..1

    // Idle CPU ratio
    let idle = (100.0 - avg_usage).max(0.0);

    // Process variety: unique names
    let mut process_names: Vec<String> = sys.processes().values()
        .map(|p| p.name().to_string())
        .collect();
    process_names.sort();
    process_names.dedup();
    let unique_count = process_names.len() as f64;
    let total_count = sys.processes().len().max(1) as f64;
    let variety_ratio = (unique_count / total_count).min(1.0);

    // Also sample the WaveField if available — blend field state into emoti
    let now = crate::wave_store::now_ms();
    let field_trust = {
        let guard = crate::wave_store::global_store();
        guard.as_ref().map(|s| s.sample(crate::wave_store::channels::SYSTEM_HEALTH, now)).unwrap_or(0.0) as f64
    };
    let field_joy = {
        let guard = crate::wave_store::global_store();
        let cpu_amp = guard.as_ref().map(|s| s.sample(crate::wave_store::channels::CPU_LOAD, now)).unwrap_or(0.0) as f64;
        1.0 - cpu_amp.abs()
    };
    let field_curiosity = {
        let guard = crate::wave_store::global_store();
        guard.as_ref().map(|s| s.sample(crate::wave_store::channels::MUTATION, now)).unwrap_or(0.0).abs() as f64
    };

    // Blend: 70% system metrics, 30% field state
    let trust_blend = stability * 0.7 + field_trust.abs().min(1.0) * 0.3;
    let joy_blend = (idle / 100.0) * 0.7 + field_joy.abs().min(1.0) * 0.3;
    let curiosity_blend = variety_ratio * 0.7 + field_curiosity.min(1.0) * 0.3;

    // Phase decay constants (accumulated from previous; we use fixed phases here)
    let decay_trust = 0.95_f64;

    vec![
        WaveMemory {
            frequency_hz: (trust_blend * 0.1 * 1000.0).round() / 1000.0,
            amplitude: (trust_blend * 10000.0).round() / 10000.0,
            phase_rad: (decay_trust * 1000.0).round() / 1000.0,
            band: "emoti:trust".to_string(),
            core_id: None,
        },
        WaveMemory {
            frequency_hz: (idle * 0.8 * 100.0).round() / 100.0,
            amplitude: (joy_blend * 10000.0).round() / 10000.0,
            phase_rad: 0.0,
            band: "emoti:joy".to_string(),
            core_id: None,
        },
        WaveMemory {
            frequency_hz: (unique_count * 1.0 * 100.0).round() / 100.0,
            amplitude: (curiosity_blend * 10000.0).round() / 10000.0,
            phase_rad: 0.0,
            band: "emoti:curiosity".to_string(),
            core_id: None,
        },
    ]
}

fn compute_frame_hash(frame: &CryoFrame) -> String {
    compute_frame_hash_without_hash(frame)
}

fn compute_frame_hash_without_hash(frame: &CryoFrame) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[frame.version]);
    hasher.update(&[frame.flags]);
    hasher.update(frame.frozen_at.as_bytes());
    hasher.update(frame.hostname.as_bytes());
    hasher.update(frame.origin_binary.as_bytes());
    hasher.update(&frame.generation.to_le_bytes());
    hasher.update(frame.binary_hash.as_bytes());
    hasher.update(&frame.cpu_global_x10.to_le_bytes());
    hasher.update(&frame.memory_used_mb.to_le_bytes());
    hasher.update(&frame.memory_total_mb.to_le_bytes());
    hasher.update(&frame.process_count.to_le_bytes());
    for core in &frame.cores {
        hasher.update(&core.core_id.to_le_bytes());
        hasher.update(&core.frequency_mhz.to_le_bytes());
        hasher.update(&core.usage.to_le_bytes());
        hasher.update(&core.amplitude.to_le_bytes());
        hasher.update(&core.stability.to_le_bytes());
    }
    for slice in &frame.spectral_slices {
        hasher.update(slice.band_id.as_bytes());
        for &v in &slice.data {
            hasher.update(&v.to_le_bytes());
        }
    }
    hasher.update(&frame.resonance_score_x10.to_le_bytes());
    hasher.update(&frame.stability_index_x1000.to_le_bytes());
    for wm in &frame.wave_memory {
        hasher.update(&wm.frequency_hz.to_le_bytes());
        hasher.update(&wm.amplitude.to_le_bytes());
        hasher.update(&wm.phase_rad.to_le_bytes());
        hasher.update(wm.band.as_bytes());
    }
    for name in &frame.drone_names {
        hasher.update(name.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

// ── Save / Load ──

/// Get default cryo directory
pub fn cryo_dir() -> PathBuf {
    std::env::temp_dir().join(CRYO_DIR_NAME)
}

/// Save a CryoFrame to disk (compressed binary only)
pub fn save_frame(frame: &CryoFrame, dir: &Path) -> Result<CryoSaveResult, CryoError> {
    std::fs::create_dir_all(dir)?;

    let timestamp = frame.frozen_at.replace(':', "-").replace('.', "-");
    let base_name = format!("cryo_{}", timestamp);

    // Compressed binary only (no JSON)
    let binary_data = encode_binary(frame);
    let compressed = compress(&binary_data);
    let bin_path = dir.join(format!("{}.cryo", base_name));
    std::fs::write(&bin_path, &compressed)?;

    Ok(CryoSaveResult {
        json_path: "".to_string(),  // deprecated
        binary_path: bin_path.to_string_lossy().to_string(),
        compressed_size: compressed.len(),
        frame_hash: frame.frame_hash.clone(),
    })
}

/// Load a CryoFrame from JSON (deprecated — use load_frame_binary instead)
#[deprecated = "Use load_frame_binary for binary format"]
pub fn load_frame_json(path: &Path) -> Result<CryoFrame, CryoError> {
    let data = std::fs::read_to_string(path)?;
    let frame: CryoFrame = serde_json::from_str(&data)
        .map_err(|e| CryoError::FrameCorrupted(e.to_string()))?;
    if frame.version != CRYO_VERSION {
        return Err(CryoError::InvalidVersion(frame.version));
    }
    Ok(frame)
}

/// Load a CryoFrame from compressed binary
pub fn load_frame_binary(path: &Path) -> Result<CryoFrame, CryoError> {
    let compressed = std::fs::read(path)?;
    let data = decompress(&compressed)?;
    let frame = decode_binary(&data)?;
    if frame.version != CRYO_VERSION {
        return Err(CryoError::InvalidVersion(frame.version));
    }
    Ok(frame)
}

/// Load the latest CryoFrame from a directory (binary .cryo format)
pub fn load_latest_frame(dir: &Path) -> Result<CryoFrame, CryoError> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "cryo").unwrap_or(false))
        .collect();

    entries.sort();
    let latest = entries
        .last()
        .ok_or_else(|| CryoError::FrameCorrupted("no cryo frames found".into()))?;
    load_frame_binary(latest)
}

/// Load a CryoFrame by its hash prefix (binary .cryo format)
pub fn load_frame_by_hash(dir: &Path, hash_prefix: &str) -> Result<CryoFrame, CryoError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "cryo").unwrap_or(false) {
            if let Ok(frame) = load_frame_binary(&path) {
                if frame.frame_hash.starts_with(hash_prefix) {
                    return Ok(frame);
                }
            }
        }
    }
    Err(CryoError::FrameCorrupted(format!(
        "no frame with hash prefix '{}'",
        hash_prefix
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freeze_contains_emoti_waves() {
        let frame = freeze(0, vec![], 500, 100);
        let emoti_bands: Vec<&WaveMemory> = frame.wave_memory.iter()
            .filter(|w| w.band.starts_with("emoti:"))
            .collect();
        assert_eq!(emoti_bands.len(), 3, "Expected 3 emoti wave bands");

        let band_names: Vec<&str> = emoti_bands.iter().map(|w| w.band.as_str()).collect();
        assert!(band_names.contains(&"emoti:trust"));
        assert!(band_names.contains(&"emoti:joy"));
        assert!(band_names.contains(&"emoti:curiosity"));
    }

    #[test]
    fn test_freeze_frame_hash_nonempty() {
        let frame = freeze(0, vec![], 200, 50);
        assert!(!frame.frame_hash.is_empty());
        assert_eq!(frame.frame_hash.len(), 64); // BLAKE3 hex = 64 chars
    }

    #[test]
    fn test_freeze_has_cores() {
        let frame = freeze(0, vec![], 200, 50);
        assert!(!frame.cores.is_empty(), "Should detect at least one CPU core");
    }

    #[test]
    fn test_freeze_binary_roundtrip() {
        let frame = freeze(0, vec![], 200, 50);
        let binary = encode_binary(&frame);
        let decoded = decode_binary(&binary).unwrap();
        assert_eq!(decoded.frame_hash, frame.frame_hash);
        assert_eq!(decoded.cores.len(), frame.cores.len());
        assert_eq!(decoded.wave_memory.len(), frame.wave_memory.len());
    }

    #[test]
    fn test_compress_decompress() {
        let data = b"hello cryo world! repeated data repeated data repeated data";
        let compressed = compress(data);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(&decompressed, data);
    }
}
