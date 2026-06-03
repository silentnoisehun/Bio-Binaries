/// Acoustic BFSK Modem — CryoFrame transmission over sound waves
///
/// Encodes CryoFrame data as Binary Frequency-Shift Keying (BFSK) audio
/// in WAV format. Mark=1200Hz (bit 1), Space=600Hz (bit 0), 100 baud.
/// Demodulation uses the Goertzel algorithm for efficient single-frequency
/// energy detection. No external audio dependencies — WAV I/O is hand-written.

use crate::cryo::{self, CryoFrame, CryoError};
use crate::qr_frame;
use image::RgbaImage;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;
use std::f64::consts::PI;
use std::path::Path;

// ── Constants ──

pub const SAMPLE_RATE: u32 = 8000;
pub const MARK_FREQ: f64 = 1200.0;    // "1" bit
pub const SPACE_FREQ: f64 = 600.0;    // "0" bit
pub const SYMBOL_RATE: u32 = 100;      // baud
pub const SAMPLES_PER_SYMBOL: u32 = SAMPLE_RATE / SYMBOL_RATE; // 80
pub const DEFAULT_AMPLITUDE: f64 = 0.8;

/// Preamble: 0xAA 0xAA (alternating bits for clock sync) + 0xCF 0x01 (magic)
pub const PREAMBLE: [u8; 4] = [0xAA, 0xAA, 0xCF, 0x01];

// ── Config ──

#[derive(Debug, Clone)]
pub struct AcousticConfig {
    pub sample_rate: u32,
    pub mark_freq: f64,
    pub space_freq: f64,
    pub symbol_rate: u32,
    pub amplitude: f64,
}

impl Default for AcousticConfig {
    fn default() -> Self {
        Self {
            sample_rate: SAMPLE_RATE,
            mark_freq: MARK_FREQ,
            space_freq: SPACE_FREQ,
            symbol_rate: SYMBOL_RATE,
            amplitude: DEFAULT_AMPLITUDE,
        }
    }
}

impl AcousticConfig {
    pub fn samples_per_symbol(&self) -> u32 {
        self.sample_rate / self.symbol_rate
    }
}

// ── Result / Error types ──

#[derive(Debug, Clone, Serialize)]
pub struct TxResult {
    pub wav_path: String,
    pub duration_secs: f64,
    pub sample_count: usize,
    pub payload_bytes: usize,
    pub frame_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RxResult {
    pub frame_valid: bool,
    pub payload_bytes: usize,
    pub crc_ok: bool,
    pub frame_hash: String,
}

#[derive(Debug)]
pub enum AcousticError {
    IoError(std::io::Error),
    InvalidWav(String),
    PreambleNotFound,
    CrcMismatch { expected: u16, computed: u16 },
    DecodeFailed(String),
    CryoError(CryoError),
}

impl std::fmt::Display for AcousticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::InvalidWav(msg) => write!(f, "invalid WAV: {}", msg),
            Self::PreambleNotFound => write!(f, "preamble not found in audio"),
            Self::CrcMismatch { expected, computed } =>
                write!(f, "CRC mismatch: expected={:04X} computed={:04X}", expected, computed),
            Self::DecodeFailed(msg) => write!(f, "decode failed: {}", msg),
            Self::CryoError(e) => write!(f, "cryo error: {}", e),
        }
    }
}

impl From<std::io::Error> for AcousticError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<CryoError> for AcousticError {
    fn from(e: CryoError) -> Self {
        Self::CryoError(e)
    }
}

// ── CRC-16 CCITT ──

pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

// ── Acoustic Frame ──

/// Build an acoustic frame: PREAMBLE + len(u16 BE) + payload + CRC16
pub fn build_acoustic_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut frame = Vec::with_capacity(4 + 2 + payload.len() + 2);
    frame.extend_from_slice(&PREAMBLE);
    frame.push((len >> 8) as u8);
    frame.push((len & 0xFF) as u8);
    frame.extend_from_slice(payload);
    let checksum = crc16(payload);
    frame.push((checksum >> 8) as u8);
    frame.push((checksum & 0xFF) as u8);
    frame
}

// ── BFSK Modulation ──

/// Modulate bytes into BFSK audio samples (PCM i16)
pub fn modulate_bfsk(data: &[u8], config: &AcousticConfig) -> Vec<i16> {
    let sps = config.samples_per_symbol() as usize;
    let total_bits = data.len() * 8;
    let mut samples = Vec::with_capacity(total_bits * sps);

    // Leading silence (50ms)
    let silence_samples = (config.sample_rate as f64 * 0.05) as usize;
    samples.extend(std::iter::repeat(0i16).take(silence_samples));

    let max_val = (i16::MAX as f64) * config.amplitude;

    for &byte in data {
        for bit_idx in (0..8).rev() {
            let bit = (byte >> bit_idx) & 1;
            let freq = if bit == 1 { config.mark_freq } else { config.space_freq };

            for s in 0..sps {
                let t = s as f64 / config.sample_rate as f64;
                let sample = (2.0 * PI * freq * t).sin() * max_val;
                samples.push(sample as i16);
            }
        }
    }

    // Trailing silence (50ms)
    samples.extend(std::iter::repeat(0i16).take(silence_samples));

    samples
}

// ── BFSK Demodulation (Goertzel) ──

/// Goertzel algorithm: compute energy at a specific frequency
fn goertzel_energy(samples: &[i16], freq: f64, sample_rate: u32) -> f64 {
    let n = samples.len() as f64;
    let k = (freq * n / sample_rate as f64).round();
    let w = 2.0 * PI * k / n;
    let coeff = 2.0 * w.cos();

    let mut s0: f64 = 0.0;
    let mut s1: f64 = 0.0;
    let mut s2: f64;

    for &sample in samples {
        s2 = s1;
        s1 = s0;
        s0 = (sample as f64) + coeff * s1 - s2;
    }

    // Power = s0² + s1² - coeff * s0 * s1
    s0 * s0 + s1 * s1 - coeff * s0 * s1
}

/// Demodulate BFSK samples back to bytes
pub fn demodulate_bfsk(samples: &[i16], config: &AcousticConfig) -> Vec<u8> {
    let sps = config.samples_per_symbol() as usize;
    if samples.len() < sps {
        return vec![];
    }

    let num_symbols = samples.len() / sps;
    let mut bits = Vec::with_capacity(num_symbols);

    // First pass: compute energy threshold from max observed energy
    let mut max_energy: f64 = 0.0;
    for i in 0..num_symbols {
        let start = i * sps;
        let end = (start + sps).min(samples.len());
        let window = &samples[start..end];
        let mark_e = goertzel_energy(window, config.mark_freq, config.sample_rate);
        let space_e = goertzel_energy(window, config.space_freq, config.sample_rate);
        let total = mark_e + space_e;
        if total > max_energy {
            max_energy = total;
        }
    }

    // Silence threshold: 1% of max energy — below this, skip the symbol
    let silence_threshold = max_energy * 0.01;

    for i in 0..num_symbols {
        let start = i * sps;
        let end = (start + sps).min(samples.len());
        let window = &samples[start..end];

        let mark_energy = goertzel_energy(window, config.mark_freq, config.sample_rate);
        let space_energy = goertzel_energy(window, config.space_freq, config.sample_rate);

        // Skip silence symbols (both energies below threshold)
        if mark_energy + space_energy < silence_threshold {
            continue;
        }

        bits.push(if mark_energy > space_energy { 1u8 } else { 0u8 });
    }

    // Convert bits to bytes
    let mut bytes = Vec::with_capacity(bits.len() / 8);
    for chunk in bits.chunks(8) {
        if chunk.len() < 8 {
            break;
        }
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            byte |= bit << (7 - i);
        }
        bytes.push(byte);
    }

    bytes
}

/// Find the preamble pattern in demodulated bytes, return offset after preamble
pub fn find_preamble(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < PREAMBLE.len() {
        return None;
    }
    for i in 0..=(bytes.len() - PREAMBLE.len()) {
        if bytes[i..i + PREAMBLE.len()] == PREAMBLE {
            return Some(i + PREAMBLE.len());
        }
    }
    None
}

/// Extract acoustic frame payload from demodulated bytes (after preamble offset)
pub fn extract_frame(bytes: &[u8], offset: usize) -> Result<(Vec<u8>, u16), AcousticError> {
    if bytes.len() < offset + 2 {
        return Err(AcousticError::DecodeFailed("not enough data for length".into()));
    }

    let payload_len = ((bytes[offset] as u16) << 8) | (bytes[offset + 1] as u16);
    let payload_start = offset + 2;
    let payload_end = payload_start + payload_len as usize;
    let crc_end = payload_end + 2;

    if bytes.len() < crc_end {
        return Err(AcousticError::DecodeFailed(format!(
            "truncated: need {} bytes, have {}",
            crc_end,
            bytes.len()
        )));
    }

    let payload = &bytes[payload_start..payload_end];
    let received_crc = ((bytes[payload_end] as u16) << 8) | (bytes[payload_end + 1] as u16);
    let computed_crc = crc16(payload);

    if received_crc != computed_crc {
        return Err(AcousticError::CrcMismatch {
            expected: received_crc,
            computed: computed_crc,
        });
    }

    Ok((payload.to_vec(), received_crc))
}

// ── WAV I/O ──

/// Write PCM i16 mono samples to a WAV file (44-byte RIFF header)
pub fn write_wav(path: &Path, samples: &[i16], sample_rate: u32) -> Result<(), AcousticError> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + samples.len() * 2);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());       // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());        // PCM format
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    for &sample in samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    std::fs::write(path, &buf)?;
    Ok(())
}

/// Read PCM i16 mono samples from a WAV file
pub fn read_wav(path: &Path) -> Result<(Vec<i16>, u32), AcousticError> {
    let data = std::fs::read(path)?;

    if data.len() < 44 {
        return Err(AcousticError::InvalidWav("file too small".into()));
    }

    // RIFF check
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(AcousticError::InvalidWav("not a RIFF/WAVE file".into()));
    }

    // Parse fmt chunk
    if &data[12..16] != b"fmt " {
        return Err(AcousticError::InvalidWav("fmt chunk not found".into()));
    }

    let audio_format = u16::from_le_bytes([data[20], data[21]]);
    if audio_format != 1 {
        return Err(AcousticError::InvalidWav(format!(
            "not PCM format (got {})", audio_format
        )));
    }

    let num_channels = u16::from_le_bytes([data[22], data[23]]);
    let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    let bits_per_sample = u16::from_le_bytes([data[34], data[35]]);

    if bits_per_sample != 16 {
        return Err(AcousticError::InvalidWav(format!(
            "expected 16-bit, got {}-bit", bits_per_sample
        )));
    }

    // Find data chunk (may not be at offset 36 if extra fmt data exists)
    let mut data_offset = 36;
    loop {
        if data_offset + 8 > data.len() {
            return Err(AcousticError::InvalidWav("data chunk not found".into()));
        }
        if &data[data_offset..data_offset + 4] == b"data" {
            break;
        }
        // Skip unknown chunk
        let chunk_size = u32::from_le_bytes([
            data[data_offset + 4], data[data_offset + 5],
            data[data_offset + 6], data[data_offset + 7],
        ]) as usize;
        data_offset += 8 + chunk_size;
    }

    let data_size = u32::from_le_bytes([
        data[data_offset + 4], data[data_offset + 5],
        data[data_offset + 6], data[data_offset + 7],
    ]) as usize;
    let pcm_start = data_offset + 8;
    let pcm_end = (pcm_start + data_size).min(data.len());

    let bytes_per_frame = num_channels as usize * 2; // 16-bit
    let mut samples = Vec::with_capacity(data_size / 2);

    let mut i = pcm_start;
    while i + 1 < pcm_end {
        let sample = i16::from_le_bytes([data[i], data[i + 1]]);
        samples.push(sample);
        // If stereo, skip extra channels (take only first channel)
        i += bytes_per_frame;
    }

    Ok((samples, sample_rate))
}

// ── High-level pipeline ──

/// Encode a CryoFrame into a WAV file via BFSK modulation
pub fn encode_cryo_to_wav(
    frame: &CryoFrame,
    wav_path: &Path,
    config: &AcousticConfig,
) -> Result<TxResult, AcousticError> {
    // CryoFrame → JSON bytes → compress
    let raw = cryo::encode_binary(frame);
    let compressed = cryo::compress(&raw);

    // Build acoustic frame with preamble + CRC
    let acoustic_frame = build_acoustic_frame(&compressed);

    // Modulate to audio
    let samples = modulate_bfsk(&acoustic_frame, config);

    // Write WAV
    write_wav(wav_path, &samples, config.sample_rate)?;

    let duration_secs = samples.len() as f64 / config.sample_rate as f64;

    Ok(TxResult {
        wav_path: wav_path.to_string_lossy().to_string(),
        duration_secs: (duration_secs * 100.0).round() / 100.0,
        sample_count: samples.len(),
        payload_bytes: compressed.len(),
        frame_hash: frame.frame_hash.clone(),
    })
}

/// Decode a WAV file back into a CryoFrame via BFSK demodulation
pub fn decode_wav_to_cryo(
    wav_path: &Path,
    config: &AcousticConfig,
) -> Result<(RxResult, CryoFrame), AcousticError> {
    // Read WAV
    let (samples, _wav_rate) = read_wav(wav_path)?;

    // Demodulate
    let bytes = demodulate_bfsk(&samples, config);

    // Find preamble
    let offset = find_preamble(&bytes)
        .ok_or(AcousticError::PreambleNotFound)?;

    // Extract frame
    let (payload, _crc) = extract_frame(&bytes, offset)?;

    // Decompress → decode CryoFrame
    let raw = cryo::decompress(&payload)?;
    let frame = cryo::decode_binary(&raw)?;

    let result = RxResult {
        frame_valid: cryo::verify_frame(&frame),
        payload_bytes: payload.len(),
        crc_ok: true,
        frame_hash: frame.frame_hash.clone(),
    };

    Ok((result, frame))
}

/// Encode raw bytes (already compressed cryo binary) into WAV
pub fn encode_bytes_to_wav(
    data: &[u8],
    wav_path: &Path,
    config: &AcousticConfig,
) -> Result<TxResult, AcousticError> {
    let acoustic_frame = build_acoustic_frame(data);
    let samples = modulate_bfsk(&acoustic_frame, config);
    write_wav(wav_path, &samples, config.sample_rate)?;

    let duration_secs = samples.len() as f64 / config.sample_rate as f64;
    let hash = blake3::hash(data).to_hex().to_string();

    Ok(TxResult {
        wav_path: wav_path.to_string_lossy().to_string(),
        duration_secs: (duration_secs * 100.0).round() / 100.0,
        sample_count: samples.len(),
        payload_bytes: data.len(),
        frame_hash: hash,
    })
}

// ── Spectrogram ──

/// Generate a spectrogram image from PCM i16 samples via STFT
///
/// Uses FFT size 512 with Hann window. The hop size is computed so that
/// total_samples / hop == width columns. The Y axis maps frequency bins
/// (low at bottom, high at top). Horizontal marker lines are drawn at
/// 600 Hz (BFSK space) and 1200 Hz (BFSK mark).
pub fn generate_spectrogram(
    samples: &[i16],
    sample_rate: u32,
    width: u32,
    height: u32,
) -> RgbaImage {
    let fft_size: usize = 512;
    let half_fft = fft_size / 2;
    let total = samples.len();
    let hop = if total > fft_size && width > 1 {
        (total - fft_size) / (width as usize).max(1)
    } else {
        1
    }
    .max(1);

    // Precompute Hann window
    let hann: Vec<f64> = (0..fft_size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (fft_size - 1) as f64).cos()))
        .collect();

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Compute magnitude dB for each column
    let cols = width as usize;
    let mut mag_db: Vec<Vec<f64>> = Vec::with_capacity(cols);
    let mut global_min = f64::MAX;
    let mut global_max = f64::MIN;

    for col in 0..cols {
        let start = col * hop;
        let end = start + fft_size;
        if end > total {
            // Pad with zeros if we run past the end
            let mut buf: Vec<Complex<f64>> = Vec::with_capacity(fft_size);
            for i in start..end {
                let s = if i < total { samples[i] as f64 } else { 0.0 };
                buf.push(Complex::new(s * hann[i - start], 0.0));
            }
            fft.process(&mut buf);
            let col_db: Vec<f64> = (0..half_fft)
                .map(|k| {
                    let mag = (buf[k].re.powi(2) + buf[k].im.powi(2)).sqrt() / fft_size as f64;
                    20.0 * (mag + 1e-10).log10()
                })
                .collect();
            for &v in &col_db {
                if v < global_min { global_min = v; }
                if v > global_max { global_max = v; }
            }
            mag_db.push(col_db);
        } else {
            let mut buf: Vec<Complex<f64>> = (0..fft_size)
                .map(|i| Complex::new(samples[start + i] as f64 * hann[i], 0.0))
                .collect();
            fft.process(&mut buf);
            let col_db: Vec<f64> = (0..half_fft)
                .map(|k| {
                    let mag = (buf[k].re.powi(2) + buf[k].im.powi(2)).sqrt() / fft_size as f64;
                    20.0 * (mag + 1e-10).log10()
                })
                .collect();
            for &v in &col_db {
                if v < global_min { global_min = v; }
                if v > global_max { global_max = v; }
            }
            mag_db.push(col_db);
        }
    }

    // Render image — Y axis: bottom = low freq, top = high freq
    let mut img = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]));
    let range = (global_max - global_min).max(1.0);

    for (col, col_data) in mag_db.iter().enumerate() {
        for row in 0..height as usize {
            // Map row to frequency bin — row 0 is top (high freq), row height-1 is bottom (low freq)
            let freq_bin = ((height as usize - 1 - row) as f64 / height as f64 * half_fft as f64) as usize;
            let freq_bin = freq_bin.min(col_data.len().saturating_sub(1));
            let intensity = ((col_data[freq_bin] - global_min) / range) as f32;
            let color = qr_frame::heatmap_color(intensity.clamp(0.0, 1.0));
            img.put_pixel(col as u32, row as u32, color);
        }
    }

    // Draw horizontal marker lines at 600 Hz and 1200 Hz
    let freq_res = sample_rate as f64 / fft_size as f64;
    let marker_freqs = [600.0_f64, 1200.0];
    let marker_color = image::Rgba([255, 255, 255, 120]);

    for &freq in &marker_freqs {
        let bin = (freq / freq_res) as usize;
        if bin < half_fft {
            // Convert bin to row (inverted Y)
            let row = height as usize - 1 - (bin as f64 / half_fft as f64 * height as f64) as usize;
            if row < height as usize {
                for x in 0..width {
                    let existing = img.get_pixel(x, row as u32);
                    let blended = qr_frame::alpha_blend(existing, marker_color);
                    img.put_pixel(x, row as u32, blended);
                }
                // Label
                let label = format!("{}Hz", freq as u32);
                qr_frame::render_text(&mut img, &label, 4, row as u32 + 1, image::Rgba([255, 255, 255, 200]));
            }
        }
    }

    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16() {
        let data = b"hello";
        let crc = crc16(data);
        assert_eq!(crc, crc16(data)); // deterministic
        assert_ne!(crc, crc16(b"world")); // different data → different CRC
    }

    #[test]
    fn test_acoustic_frame_roundtrip() {
        let payload = b"test payload 1234";
        let frame = build_acoustic_frame(payload);

        // Should start with preamble
        assert_eq!(&frame[0..4], &PREAMBLE);

        // Length
        let len = ((frame[4] as u16) << 8) | (frame[5] as u16);
        assert_eq!(len, payload.len() as u16);

        // Extract
        let offset = find_preamble(&frame).unwrap();
        let (decoded, _crc) = extract_frame(&frame, offset).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_bfsk_modulation_demodulation() {
        let config = AcousticConfig::default();
        let data = vec![0xAA, 0x55, 0xFF, 0x00];
        let samples = modulate_bfsk(&data, &config);
        let decoded = demodulate_bfsk(&samples, &config);

        // With silence filtering, decoded should match data directly
        assert_eq!(decoded.len(), data.len());
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_preamble_detection() {
        let mut bytes = vec![0x00, 0x00, 0x00]; // noise
        bytes.extend_from_slice(&PREAMBLE);
        bytes.extend_from_slice(&[0x00, 0x04, 0x01, 0x02, 0x03, 0x04]); // len + payload
        let offset = find_preamble(&bytes);
        assert_eq!(offset, Some(3 + PREAMBLE.len()));
    }
}
