/// WaveStore — Persistent wave packet storage with interference and decay.
///
/// Foundation of the WaveField system. Stores time-decaying wave packets,
/// computes real-time superposition (constructive/destructive interference),
/// and persists to disk as binary format (no JSON).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single wave packet in the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavePacket {
    /// Unique packet ID (monotonic counter)
    pub id: u64,
    /// Emission timestamp (milliseconds since UNIX epoch)
    pub emitted_at: u64,
    /// Frequency channel (Hz) — determines which "band" this packet lives in
    pub frequency: f32,
    /// Peak amplitude at emission time
    pub amplitude: f32,
    /// Decay rate per millisecond (amplitude *= e^(-decay * dt))
    pub decay: f32,
    /// Phase offset in radians
    pub phase: f32,
    /// Who emitted this packet
    pub origin: WaveOrigin,
    /// Optional tag for downstream consumers
    pub tag: Option<String>,
}

impl Default for WavePacket {
    fn default() -> Self {
        Self {
            id: 0,
            emitted_at: now_ms(),
            frequency: 0.0,
            amplitude: 0.0,
            decay: 0.01,
            phase: 0.0,
            origin: WaveOrigin::External,
            tag: None,
        }
    }
}

impl WavePacket {
    /// Compute current amplitude at time t_now (exponential decay)
    pub fn amplitude_at(&self, t_now: u64) -> f32 {
        if t_now <= self.emitted_at {
            return self.amplitude;
        }
        let dt_ms = (t_now - self.emitted_at) as f32;
        self.amplitude * (-self.decay * dt_ms).exp()
    }

    /// Is this packet effectively dead? (amplitude < threshold)
    pub fn is_dead(&self, t_now: u64, threshold: f32) -> bool {
        self.amplitude_at(t_now).abs() < threshold
    }

    /// Instantaneous wave value: A(t) * sin(2π·f·t + φ)
    pub fn sample_at(&self, t_now: u64) -> f32 {
        let amp = self.amplitude_at(t_now);
        if amp.abs() < 1e-6 {
            return 0.0;
        }
        let dt_sec = (t_now - self.emitted_at) as f32 / 1000.0;
        amp * (2.0 * std::f32::consts::PI * self.frequency * dt_sec + self.phase).sin()
    }
}

/// Origin of a wave packet — who emitted it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WaveOrigin {
    /// Emitted by the WaveField itself (emergent behavior)
    FieldEmergence,
    /// Emitted by mutation-sentinel
    MutationSentinel,
    /// Emitted by immune system
    ImmuneAlert,
    /// Emitted by cryo subsystem
    Cryo,
    /// Emitted externally (CLI inject, test, etc.)
    External,
    /// Emitted by vagus-nerve (internal organ sensing: CPU, RAM, disk)
    VagusNerve,
    /// Emitted by a specific binary
    Binary(String),
}

/// Interference pattern classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterferencePattern {
    /// Waves reinforce each other (positive sum)
    Constructive,
    /// Waves cancel each other (near-zero sum)
    Destructive,
    /// High variance, unpredictable (many conflicting waves)
    Chaotic,
    /// No significant wave activity
    Silent,
}

/// Interference analysis result for a frequency band
#[derive(Debug, Clone)]
pub struct InterferenceResult {
    /// Classified pattern
    pub pattern: InterferencePattern,
    /// Combined amplitude (superposition)
    pub combined_amplitude: f32,
    /// Number of active packets in this band
    pub active_count: usize,
    /// Variance of individual amplitudes
    pub variance: f32,
}

// ── WaveStore ──

/// Persistent wave packet store with interference computation.
pub struct WaveStore {
    packets: Vec<WavePacket>,
    store_path: PathBuf,
    max_packets: usize,
    next_id: u64,
}

const DEAD_THRESHOLD: f32 = 0.001;
const FREQ_TOLERANCE: f32 = 0.5; // Hz — packets within ±0.5 Hz are in the same band

impl WaveStore {
    /// Create a new empty store.
    pub fn new(store_path: PathBuf, max_packets: usize) -> Self {
        Self {
            packets: Vec::new(),
            store_path,
            max_packets,
            next_id: 1,
        }
    }

    /// Create a new store with default path and capacity.
    pub fn new_default() -> Self {
        let path = default_path();
        Self::new(path, 10000)
    }

    /// Emit a new wave packet into the field. Returns the packet ID.
    pub fn emit(&mut self, mut packet: WavePacket) -> u64 {
        packet.id = self.next_id;
        self.next_id += 1;
        if packet.emitted_at == 0 {
            packet.emitted_at = now_ms();
        }
        let id = packet.id;
        self.packets.push(packet);

        // Enforce capacity
        if self.packets.len() > self.max_packets {
            // Remove oldest dead packets first, then oldest alive
            let now = now_ms();
            self.packets.retain(|p| !p.is_dead(now, DEAD_THRESHOLD));
            if self.packets.len() > self.max_packets {
                let drain_count = self.packets.len() - self.max_packets;
                self.packets.drain(0..drain_count);
            }
        }

        id
    }

    /// Sample the superposition of all waves at a given frequency at time t_now.
    /// Returns the combined waveform value (sum of all matching packets' sin waves).
    pub fn sample(&self, freq: f32, t_now: u64) -> f32 {
        self.packets
            .iter()
            .filter(|p| (p.frequency - freq).abs() < FREQ_TOLERANCE)
            .map(|p| p.sample_at(t_now))
            .sum()
    }

    /// Sample just the combined amplitude envelope (ignoring phase) for a frequency.
    pub fn amplitude_at(&self, freq: f32, t_now: u64) -> f32 {
        self.packets
            .iter()
            .filter(|p| (p.frequency - freq).abs() < FREQ_TOLERANCE)
            .map(|p| p.amplitude_at(t_now))
            .sum()
    }

    /// Get energy map: frequency → total amplitude across all active packets.
    pub fn energy_map(&self, t_now: u64) -> HashMap<u32, f32> {
        let mut map: HashMap<u32, f32> = HashMap::new();
        for p in &self.packets {
            let amp = p.amplitude_at(t_now).abs();
            if amp < DEAD_THRESHOLD {
                continue;
            }
            let key = p.frequency.round() as u32;
            *map.entry(key).or_insert(0.0) += amp;
        }
        map
    }

    /// Remove dead packets (amplitude below threshold).
    pub fn decay_pass(&mut self, t_now: u64) {
        self.packets.retain(|p| !p.is_dead(t_now, DEAD_THRESHOLD));
    }

    /// Compute interference pattern for a specific frequency band.
    pub fn interference_score(&self, freq: f32, t_now: u64) -> InterferenceResult {
        let matching: Vec<&WavePacket> = self.packets
            .iter()
            .filter(|p| (p.frequency - freq).abs() < FREQ_TOLERANCE)
            .filter(|p| p.amplitude_at(t_now).abs() >= DEAD_THRESHOLD)
            .collect();

        let count = matching.len();
        if count == 0 {
            return InterferenceResult {
                pattern: InterferencePattern::Silent,
                combined_amplitude: 0.0,
                active_count: 0,
                variance: 0.0,
            };
        }

        let amplitudes: Vec<f32> = matching.iter().map(|p| p.amplitude_at(t_now)).collect();
        let combined: f32 = amplitudes.iter().sum();
        let abs_sum: f32 = amplitudes.iter().map(|a| a.abs()).sum();
        let mean = combined / count as f32;
        let variance = amplitudes.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / count as f32;

        let pattern = if count == 1 {
            if combined.abs() >= DEAD_THRESHOLD {
                InterferencePattern::Constructive
            } else {
                InterferencePattern::Silent
            }
        } else if abs_sum > DEAD_THRESHOLD && combined.abs() / abs_sum > 0.7 {
            // Most energy is in the same direction → constructive
            InterferencePattern::Constructive
        } else if abs_sum > DEAD_THRESHOLD && combined.abs() / abs_sum < 0.2 {
            // Energy cancels out → destructive
            InterferencePattern::Destructive
        } else if variance > 0.1 && count >= 3 {
            InterferencePattern::Chaotic
        } else {
            InterferencePattern::Constructive
        };

        InterferenceResult {
            pattern,
            combined_amplitude: combined,
            active_count: count,
            variance,
        }
    }

    /// How long (ms) has the given frequency been silent?
    /// Returns 0 if there are currently active packets.
    pub fn silence_duration(&self, freq: f32, t_now: u64) -> u64 {
        let latest_active = self.packets
            .iter()
            .filter(|p| (p.frequency - freq).abs() < FREQ_TOLERANCE)
            .filter(|p| !p.is_dead(t_now, DEAD_THRESHOLD))
            .map(|p| p.emitted_at)
            .max();

        match latest_active {
            Some(_) => 0, // Still active
            None => {
                // Find when the last packet on this freq died (or was emitted)
                let latest_emit = self.packets
                    .iter()
                    .filter(|p| (p.frequency - freq).abs() < FREQ_TOLERANCE)
                    .map(|p| p.emitted_at)
                    .max();
                match latest_emit {
                    Some(t) => t_now.saturating_sub(t),
                    None => u64::MAX, // Never had any packet
                }
            }
        }
    }

    /// Total number of live packets.
    pub fn live_count(&self, t_now: u64) -> usize {
        self.packets.iter().filter(|p| !p.is_dead(t_now, DEAD_THRESHOLD)).count()
    }

    /// All currently active packets (snapshot).
    pub fn active_packets(&self, t_now: u64) -> Vec<&WavePacket> {
        self.packets.iter().filter(|p| !p.is_dead(t_now, DEAD_THRESHOLD)).collect()
    }

    /// Persist store to disk (binary format).
    pub fn persist(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = encode_packets(&self.packets);
        std::fs::write(&self.store_path, data)
    }

    /// Load store from disk (binary format).
    pub fn load(path: &Path, max_packets: usize) -> Result<Self, std::io::Error> {
        let data = std::fs::read(path)?;
        let packets = decode_packets(&data)?;
        let next_id = packets.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        Ok(Self {
            packets,
            store_path: path.to_path_buf(),
            max_packets,
            next_id,
        })
    }

    /// Inbox path — external injectors write here, daemon reads and clears (binary format).
    pub fn inbox_path(&self) -> PathBuf {
        self.store_path.with_extension("inbox.bin")
    }

    /// Merge packets from inbox file (written by inject command).
    /// Reads inbox, assigns fresh IDs, appends to our packets, then clears inbox.
    pub fn merge_inbox(&mut self) {
        let inbox = self.inbox_path();
        let data = match std::fs::read(&inbox) {
            Ok(d) if !d.is_empty() => d,
            _ => return,
        };
        let incoming = match decode_packets(&data) {
            Ok(p) => p,
            Err(_) => return,
        };
        if incoming.is_empty() {
            return;
        }
        for mut p in incoming {
            p.id = self.next_id;
            self.next_id += 1;
            self.packets.push(p);
        }
        // Clear inbox: write empty encoded list
        let _ = std::fs::write(&inbox, encode_packets(&[]));
    }

    /// Append a packet to the inbox file (used by inject command when daemon is running).
    pub fn append_to_inbox(store_path: &Path, packet: &WavePacket) -> Result<(), std::io::Error> {
        let inbox = store_path.with_extension("inbox.bin");
        if let Some(parent) = inbox.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut packets: Vec<WavePacket> = std::fs::read(&inbox)
            .ok()
            .and_then(|d| decode_packets(&d).ok())
            .unwrap_or_default();
        packets.push(packet.clone());
        let data = encode_packets(&packets);
        std::fs::write(&inbox, data)
    }

    /// Store file path.
    pub fn path(&self) -> &Path {
        &self.store_path
    }
}

// ── Time helper ──

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Binary encoding/decoding ──

/// Encode packets to binary format: [magic:4][count:4] + packets
fn encode_packets(packets: &[WavePacket]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"WAVE");  // magic
    buf.extend_from_slice(&(packets.len() as u32).to_le_bytes());
    for packet in packets {
        encode_packet(&mut buf, packet);
    }
    buf
}

/// Decode binary format back to packets
fn decode_packets(data: &[u8]) -> Result<Vec<WavePacket>, std::io::Error> {
    if data.len() < 8 {
        return Ok(vec![]);
    }
    if &data[0..4] != b"WAVE" {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid magic"));
    }
    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut packets = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        if offset >= data.len() {
            break;
        }
        match decode_packet(&data[offset..]) {
            Some((packet, consumed)) => {
                packets.push(packet);
                offset += consumed;
            }
            None => break,
        }
    }
    Ok(packets)
}

/// Encode a single packet
fn encode_packet(buf: &mut Vec<u8>, p: &WavePacket) {
    buf.extend_from_slice(&p.id.to_le_bytes());
    buf.extend_from_slice(&p.emitted_at.to_le_bytes());
    buf.extend_from_slice(&p.frequency.to_le_bytes());
    buf.extend_from_slice(&p.amplitude.to_le_bytes());
    buf.extend_from_slice(&p.decay.to_le_bytes());
    buf.extend_from_slice(&p.phase.to_le_bytes());

    // Encode origin
    let (origin_type, origin_str) = match &p.origin {
        WaveOrigin::FieldEmergence => (0u8, ""),
        WaveOrigin::MutationSentinel => (1u8, ""),
        WaveOrigin::ImmuneAlert => (2u8, ""),
        WaveOrigin::Cryo => (3u8, ""),
        WaveOrigin::External => (4u8, ""),
        WaveOrigin::VagusNerve => (6u8, ""),
        WaveOrigin::Binary(s) => (5u8, s.as_str()),
    };
    buf.push(origin_type);
    buf.extend_from_slice(&(origin_str.len() as u16).to_le_bytes());
    buf.extend_from_slice(origin_str.as_bytes());

    // Encode tag
    let tag_bytes = p.tag.as_ref().map(|s| s.as_bytes()).unwrap_or(&[]);
    buf.extend_from_slice(&(tag_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(tag_bytes);
}

/// Decode a single packet, return (packet, bytes_consumed)
fn decode_packet(data: &[u8]) -> Option<(WavePacket, usize)> {
    let mut offset = 0;

    // Read fixed fields (42 bytes)
    if offset + 42 > data.len() {
        return None;
    }
    let id = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?);
    offset += 8;
    let emitted_at = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?);
    offset += 8;
    let frequency = f32::from_le_bytes(data[offset..offset+4].try_into().ok()?);
    offset += 4;
    let amplitude = f32::from_le_bytes(data[offset..offset+4].try_into().ok()?);
    offset += 4;
    let decay = f32::from_le_bytes(data[offset..offset+4].try_into().ok()?);
    offset += 4;
    let phase = f32::from_le_bytes(data[offset..offset+4].try_into().ok()?);
    offset += 4;

    // Read origin
    let origin_type = data[offset];
    offset += 1;
    let origin_len = u16::from_le_bytes(data[offset..offset+2].try_into().ok()?) as usize;
    offset += 2;

    if offset + origin_len > data.len() {
        return None;
    }
    let origin_str = std::str::from_utf8(&data[offset..offset+origin_len]).ok()?;
    offset += origin_len;

    let origin = match origin_type {
        0 => WaveOrigin::FieldEmergence,
        1 => WaveOrigin::MutationSentinel,
        2 => WaveOrigin::ImmuneAlert,
        3 => WaveOrigin::Cryo,
        4 => WaveOrigin::External,
        5 => WaveOrigin::Binary(origin_str.to_string()),
        6 => WaveOrigin::VagusNerve,
        _ => return None,
    };

    // Read tag
    if offset + 2 > data.len() {
        return None;
    }
    let tag_len = u16::from_le_bytes(data[offset..offset+2].try_into().ok()?) as usize;
    offset += 2;

    if offset + tag_len > data.len() {
        return None;
    }
    let tag = if tag_len > 0 {
        Some(std::str::from_utf8(&data[offset..offset+tag_len]).ok()?.to_string())
    } else {
        None
    };
    offset += tag_len;

    Some((
        WavePacket { id, emitted_at, frequency, amplitude, decay, phase, origin, tag },
        offset,
    ))
}

pub fn default_path() -> PathBuf {
    std::env::temp_dir().join(".bio-wavefield").join("wave_store.bin")
}

// ── Default frequency channels ──

/// Well-known frequency channels used across the system.
pub mod channels {
    pub const SYSTEM_HEALTH: f32 = 4.0;
    pub const CPU_LOAD: f32 = 12.0;
    pub const MUTATION: f32 = 28.0;
    pub const SECURITY: f32 = 60.0;
    pub const ACOUSTIC: f32 = 100.0;
    pub const FEVER: f32 = 37.0;    // CPU thermal state (biologically inspired: body temperature)
    pub const DREAM: f32 = 0.5;     // Dream/sleep loop — slow background pulse
    pub const TRUST: f32 = 8.0;     // Emergent trust — builds during stability, raises tolerance
    pub const SYMBIOSIS: f32 = 16.0; // Octave harmony — binaries share state, co-regulate
    pub const FLOW: f32 = 2.0;      // Flow state marker — 4+8+16Hz octave resonance detected
}

// ── Global WaveStore access ──

use std::sync::Mutex;

/// Macro-free global: initialized on first access.
static GLOBAL_STORE: Mutex<Option<WaveStore>> = Mutex::new(None);

/// Get or initialize the global WaveStore.
pub fn global_store() -> std::sync::MutexGuard<'static, Option<WaveStore>> {
    let mut guard = GLOBAL_STORE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let path = default_path();
        let store = if path.exists() {
            WaveStore::load(&path, 10_000).unwrap_or_else(|_| WaveStore::new(path.clone(), 10_000))
        } else {
            WaveStore::new(path, 10_000)
        };
        *guard = Some(store);
    }
    guard
}

/// Convenience: emit into the global store.
pub fn global_emit(packet: WavePacket) -> u64 {
    let mut guard = global_store();
    guard.as_mut().map(|s| s.emit(packet)).unwrap_or(0)
}

/// Convenience: sample the global store.
pub fn global_sample(freq: f32) -> f32 {
    let guard = global_store();
    guard.as_ref().map(|s| s.sample(freq, now_ms())).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> WaveStore {
        WaveStore::new(std::env::temp_dir().join(".bio-test-wavestore.bin"), 1000)
    }

    #[test]
    fn test_wavestore_emit_and_sample() {
        let mut store = test_store();
        let t = now_ms();
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 12.0,
            amplitude: 1.0,
            decay: 0.0001,
            phase: 0.0,
            origin: WaveOrigin::External,
            ..Default::default()
        });
        // Amplitude should be near 1.0 right after emission
        let amp = store.amplitude_at(12.0, t);
        assert!(amp > 0.99, "amplitude should be ~1.0, got {}", amp);
    }

    #[test]
    fn test_wavestore_decay_to_zero() {
        let mut store = test_store();
        let t = now_ms();
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 28.0,
            amplitude: 1.0,
            decay: 0.01, // fast decay
            ..Default::default()
        });
        // After 1000ms with decay=0.01: e^(-0.01*1000) = e^(-10) ≈ 0.00005
        let amp = store.amplitude_at(28.0, t + 1000);
        assert!(amp < 0.001, "should have decayed, got {}", amp);
    }

    #[test]
    fn test_wavestore_constructive_interference() {
        let mut store = test_store();
        let t = now_ms();
        // Two positive waves on same frequency
        for _ in 0..2 {
            store.emit(WavePacket {
                emitted_at: t,
                frequency: 60.0,
                amplitude: 0.5,
                decay: 0.0001,
                ..Default::default()
            });
        }
        let result = store.interference_score(60.0, t);
        assert_eq!(result.pattern, InterferencePattern::Constructive);
        assert!(result.combined_amplitude > 0.9);
    }

    #[test]
    fn test_wavestore_destructive_interference() {
        let mut store = test_store();
        let t = now_ms();
        // Positive + negative cancel
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 60.0,
            amplitude: 1.0,
            decay: 0.0001,
            ..Default::default()
        });
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 60.0,
            amplitude: -1.0,
            decay: 0.0001,
            ..Default::default()
        });
        let result = store.interference_score(60.0, t);
        assert_eq!(result.pattern, InterferencePattern::Destructive);
        assert!(result.combined_amplitude.abs() < 0.1);
    }

    #[test]
    fn test_wavestore_silence_duration() {
        let mut store = test_store();
        let t = now_ms();
        // Emit a fast-decaying packet in the past
        store.emit(WavePacket {
            emitted_at: t.saturating_sub(10_000),
            frequency: 4.0,
            amplitude: 0.5,
            decay: 0.01,
            ..Default::default()
        });
        let silence = store.silence_duration(4.0, t);
        assert!(silence >= 10_000, "should be silent for >=10s, got {}ms", silence);
    }

    #[test]
    fn test_wavestore_roundtrip() {
        let path = std::env::temp_dir().join(".bio-test-wavestore-rt.bin");
        let mut store = WaveStore::new(path.clone(), 1000);
        store.emit(WavePacket {
            frequency: 12.0,
            amplitude: 0.77,
            decay: 0.005,
            origin: WaveOrigin::MutationSentinel,
            ..Default::default()
        });
        store.emit(WavePacket {
            frequency: 60.0,
            amplitude: -0.5,
            decay: 0.002,
            origin: WaveOrigin::ImmuneAlert,
            ..Default::default()
        });
        store.persist().unwrap();

        let loaded = WaveStore::load(&path, 1000).unwrap();
        assert_eq!(loaded.packets.len(), 2);
        assert_eq!(loaded.packets[0].origin, WaveOrigin::MutationSentinel);
        assert_eq!(loaded.packets[1].origin, WaveOrigin::ImmuneAlert);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_wavestore_energy_map() {
        let mut store = test_store();
        let t = now_ms();
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 4.0,
            amplitude: 0.8,
            decay: 0.0001,
            ..Default::default()
        });
        store.emit(WavePacket {
            emitted_at: t,
            frequency: 60.0,
            amplitude: 0.3,
            decay: 0.0001,
            ..Default::default()
        });
        let map = store.energy_map(t);
        assert!(map.get(&4).unwrap_or(&0.0) > &0.7);
        assert!(map.get(&60).unwrap_or(&0.0) > &0.2);
    }
}
