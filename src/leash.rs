/// Digital Leash — Safety mechanisms to prevent runaway replication
///
/// Three lines of defense:
/// 1. Metabolic Rate Limiter — CPU/memory-based throttle on replication
/// 2. Token-based Authorization — Queen-issued tokens required for reproduction
/// 3. Apoptosis — Emergency self-destruct on Queen's command

use crate::auth::DroneToken;
use crate::bio_protocol::{BioMessage, BioOp, MAX_GENERATION};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use std::sync::atomic::{AtomicBool, Ordering};

// ── Global apoptosis signal ──
static APOPTOSIS_TRIGGERED: AtomicBool = AtomicBool::new(false);

// ── Global cryostasis signal ──
static CRYOSTASIS_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Check if cryostasis is active
pub fn is_cryostasis() -> bool {
    CRYOSTASIS_ACTIVE.load(Ordering::SeqCst)
}

/// Activate cryostasis — block replication
pub fn activate_cryostasis() {
    CRYOSTASIS_ACTIVE.store(true, Ordering::SeqCst);
}

/// Deactivate cryostasis — resume operations
pub fn deactivate_cryostasis() {
    CRYOSTASIS_ACTIVE.store(false, Ordering::SeqCst);
}

/// Check if apoptosis has been triggered
pub fn is_apoptosis() -> bool {
    APOPTOSIS_TRIGGERED.load(Ordering::SeqCst)
}

/// Trigger apoptosis — after this, the process should terminate
pub fn trigger_apoptosis() {
    APOPTOSIS_TRIGGERED.store(true, Ordering::SeqCst);
}

// ── 1. Metabolic Rate Limiter ──

#[derive(Debug, Clone)]
pub struct MetabolicLimits {
    /// Max CPU usage % before replication is blocked
    pub max_cpu_percent: f32,
    /// Max memory usage % before replication is blocked
    pub max_memory_percent: f64,
    /// Maximum generation depth
    pub max_generation: u32,
    /// Minimum seconds between replications
    pub cooldown_secs: u64,
}

impl Default for MetabolicLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 80.0,
            max_memory_percent: 85.0,
            max_generation: MAX_GENERATION,
            cooldown_secs: 30,
        }
    }
}

pub struct MetabolicGate {
    pub limits: MetabolicLimits,
    last_replication: std::time::Instant,
}

impl MetabolicGate {
    pub fn new(limits: MetabolicLimits) -> Self {
        Self {
            limits,
            last_replication: std::time::Instant::now().checked_sub(std::time::Duration::from_secs(9999)).unwrap_or(std::time::Instant::now()),
        }
    }

    /// Check if replication is currently allowed
    pub fn can_replicate(&self, generation: u32) -> Result<(), LeashDenial> {
        // Check apoptosis
        if is_apoptosis() {
            return Err(LeashDenial::ApoptosisActive);
        }

        // Check cryostasis
        if is_cryostasis() {
            return Err(LeashDenial::CryostasisActive);
        }

        // Check generation limit
        if generation >= self.limits.max_generation {
            return Err(LeashDenial::GenerationLimit {
                current: generation,
                max: self.limits.max_generation,
            });
        }

        // Check cooldown
        let elapsed = self.last_replication.elapsed().as_secs();
        if elapsed < self.limits.cooldown_secs {
            return Err(LeashDenial::Cooldown {
                remaining_secs: self.limits.cooldown_secs - elapsed,
            });
        }

        // Check system resources
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu();
        sys.refresh_memory();

        let cpu = sys.global_cpu_info().cpu_usage();
        if cpu > self.limits.max_cpu_percent {
            return Err(LeashDenial::CpuOverload {
                current: cpu,
                limit: self.limits.max_cpu_percent,
            });
        }

        let mem_total = sys.total_memory() as f64;
        let mem_used = sys.used_memory() as f64;
        let mem_pct = if mem_total > 0.0 { (mem_used / mem_total) * 100.0 } else { 0.0 };
        if mem_pct > self.limits.max_memory_percent {
            return Err(LeashDenial::MemoryOverload {
                current: mem_pct,
                limit: self.limits.max_memory_percent,
            });
        }

        Ok(())
    }

    /// Record that a replication occurred
    pub fn record_replication(&mut self) {
        self.last_replication = std::time::Instant::now();
    }
}

// ── 2. Token Verifier ──

pub struct TokenVerifier {
    token: Option<DroneToken>,
}

impl TokenVerifier {
    pub fn new() -> Self {
        Self { token: None }
    }

    pub fn set_token(&mut self, token: DroneToken) {
        self.token = Some(token);
    }

    pub fn has_valid_token(&self) -> bool {
        match &self.token {
            Some(t) => !t.is_expired(),
            None => false,
        }
    }

    pub fn check_token(&self) -> Result<&DroneToken, LeashDenial> {
        match &self.token {
            None => Err(LeashDenial::NoToken),
            Some(t) if t.is_expired() => Err(LeashDenial::TokenExpired),
            Some(t) => Ok(t),
        }
    }
}

// ── 3. Apoptosis Handler ──

pub struct ApoptosisHandler;

impl ApoptosisHandler {
    /// Execute apoptosis — self-destruct sequence
    pub fn execute(reason: &str) {
        eprintln!("[APOPTOSIS] Triggered: {}", reason);
        trigger_apoptosis();

        // Clean up temp files
        let temp_dir = std::env::temp_dir().join("bio-entangle");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let token_dir = std::env::temp_dir().join("bio-tokens");
        let _ = std::fs::remove_dir_all(&token_dir);

        // If this is a clone (generation > 0), delete self
        // We check by looking at the binary name for "-gen" pattern
        if let Ok(exe) = std::env::current_exe() {
            let name = exe.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.contains("-gen") {
                eprintln!("[APOPTOSIS] Clone detected ({}). Self-deleting.", name);
                // On Windows, can't delete running exe directly.
                // Schedule deletion after exit.
                #[cfg(windows)]
                {
                    let path = exe.to_string_lossy().to_string();
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "ping", "127.0.0.1", "-n", "2", ">", "nul", "&", "del", &path])
                        .spawn();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::fs::remove_file(&exe);
                }
            }
        }

        eprintln!("[APOPTOSIS] Cleanup complete. Terminating.");
        std::process::exit(0);
    }

    /// Check incoming message for apoptosis signal
    pub fn check_message(msg: &BioMessage) -> bool {
        if msg.op == BioOp::Apoptosis {
            Self::execute("APOPTOSIS command received from Queen");
            true
        } else if msg.op == BioOp::Shutdown {
            Self::execute("SHUTDOWN command received");
            true
        } else {
            false
        }
    }
}

// ── 4. Cryostasis Handler ──

pub struct CryostasisHandler;

impl CryostasisHandler {
    /// Execute freeze — capture spectral snapshot
    pub fn execute_freeze(
        reason: &str,
        generation: u32,
        drone_names: Vec<String>,
    ) -> Result<String, String> {
        eprintln!("[CRYO] Freeze triggered: {}", reason);
        activate_cryostasis();

        let frame = crate::cryo::freeze(generation, drone_names, 1000, 200);
        let dir = crate::cryo::cryo_dir();
        match crate::cryo::save_frame(&frame, &dir) {
            Ok(result) => {
                // Generate Frame Snap PNG
                let png_data = crate::qr_frame::generate_frame_snap(&frame);
                let timestamp = frame.frozen_at.replace(':', "-").replace('.', "-");
                let png_path = dir.join(format!("cryo_{}.png", timestamp));
                let _ = std::fs::write(&png_path, &png_data);

                eprintln!("[CRYO] Frame saved: {} ({} bytes compressed)",
                    result.frame_hash[..16].to_string(), result.compressed_size);
                Ok(result.frame_hash)
            }
            Err(e) => {
                deactivate_cryostasis();
                Err(format!("freeze failed: {}", e))
            }
        }
    }

    /// Execute thaw — compare frozen state with current
    pub fn execute_thaw(frame_hash: Option<&str>) -> Result<crate::cryo::ThawReport, String> {
        let dir = crate::cryo::cryo_dir();
        let frame = match frame_hash {
            Some(hash) => crate::cryo::load_frame_by_hash(&dir, hash)
                .map_err(|e| format!("load by hash: {}", e))?,
            None => crate::cryo::load_latest_frame(&dir)
                .map_err(|e| format!("load latest: {}", e))?,
        };

        let report = crate::cryo::thaw(&frame);
        deactivate_cryostasis();

        eprintln!("[CRYO] Thaw complete: status={} drift={:.2} correlation={:.3}",
            report.thaw_status, report.resonance_drift, report.spectral_correlation);
        Ok(report)
    }

    /// Check incoming message for cryostasis signals
    pub fn check_message(msg: &BioMessage) -> Option<BioOp> {
        match msg.op {
            BioOp::Freeze | BioOp::Thaw | BioOp::FrozenAck => Some(msg.op),
            _ => None,
        }
    }
}

// ── 5. Immune System Handler ──

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertType {
    IntrusionDetected   = 1,
    IntegrityViolation  = 2,
    AnomalousTraffic    = 3,
    ResourceExhaustion  = 4,
}

impl AlertType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::IntrusionDetected),
            2 => Some(Self::IntegrityViolation),
            3 => Some(Self::AnomalousTraffic),
            4 => Some(Self::ResourceExhaustion),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::IntrusionDetected  => "INTRUSION_DETECTED",
            Self::IntegrityViolation => "INTEGRITY_VIOLATION",
            Self::AnomalousTraffic   => "ANOMALOUS_TRAFFIC",
            Self::ResourceExhaustion => "RESOURCE_EXHAUSTION",
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Low      = 1,
    Medium   = 2,
    High     = 3,
    Critical = 4,
}

impl AlertSeverity {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Low),
            2 => Some(Self::Medium),
            3 => Some(Self::High),
            4 => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Low      => "LOW",
            Self::Medium   => "MEDIUM",
            Self::High     => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}

pub struct ImmuneHandler;

impl ImmuneHandler {
    /// Handle an immune alert — throttle metabolic gate on high severity
    pub fn handle_alert(
        gate: &mut MetabolicGate,
        alert_type: AlertType,
        severity: AlertSeverity,
    ) {
        eprintln!("[IMMUNE] Alert: {} severity={}", alert_type.name(), severity.name());

        // Emit alert wave into the field
        let amp = match severity {
            AlertSeverity::Low => 0.2,
            AlertSeverity::Medium => 0.5,
            AlertSeverity::High => 0.8,
            AlertSeverity::Critical => 1.0,
        };
        crate::wave_store::global_emit(crate::wave_store::WavePacket {
            frequency: crate::wave_store::channels::SECURITY,
            amplitude: amp,
            decay: 0.005,
            origin: crate::wave_store::WaveOrigin::ImmuneAlert,
            tag: Some(alert_type.name().to_string()),
            ..Default::default()
        });

        // Throttle on high/critical severity
        if severity >= AlertSeverity::High {
            gate.limits.max_cpu_percent = (gate.limits.max_cpu_percent * 0.5).max(10.0);
            gate.limits.cooldown_secs = (gate.limits.cooldown_secs * 2).min(300);
            eprintln!("[IMMUNE] Throttled: max_cpu={:.0}%, cooldown={}s",
                gate.limits.max_cpu_percent, gate.limits.cooldown_secs);
        }
    }
}

// ── 6. CRISPR Patch Handler ──

pub struct CrisprHandler;

impl CrisprHandler {
    /// Handle a CRISPR patch — runtime config override or throttle adjustment
    pub fn handle_patch(
        gate: &mut MetabolicGate,
        payload: &[u8],
    ) {
        let fields = crate::bio_protocol::decode_fields(payload);

        for (key, value) in &fields {
            match key.as_str() {
                "max_cpu" => {
                    if value.len() >= 4 {
                        let v = f32::from_le_bytes([value[0], value[1], value[2], value[3]]);
                        gate.limits.max_cpu_percent = v.clamp(10.0, 100.0);
                        eprintln!("[CRISPR] max_cpu_percent → {:.0}%", gate.limits.max_cpu_percent);
                    }
                }
                "max_memory" => {
                    if value.len() >= 8 {
                        let v = f64::from_le_bytes([
                            value[0], value[1], value[2], value[3],
                            value[4], value[5], value[6], value[7],
                        ]);
                        gate.limits.max_memory_percent = v.clamp(10.0, 100.0);
                        eprintln!("[CRISPR] max_memory_percent → {:.0}%", gate.limits.max_memory_percent);
                    }
                }
                "cooldown" => {
                    if value.len() >= 8 {
                        let v = u64::from_le_bytes([
                            value[0], value[1], value[2], value[3],
                            value[4], value[5], value[6], value[7],
                        ]);
                        gate.limits.cooldown_secs = v.min(600);
                        eprintln!("[CRISPR] cooldown_secs → {}s", gate.limits.cooldown_secs);
                    }
                }
                _ => {
                    eprintln!("[CRISPR] Unknown patch key: {}", key);
                }
            }
        }
    }
}

// ── Combined Leash ──

pub struct DigitalLeash {
    pub metabolic: MetabolicGate,
    pub token: TokenVerifier,
    pub generation: u32,
    pub drone_name: String,
}

impl DigitalLeash {
    pub fn new(drone_name: &str, generation: u32) -> Self {
        Self {
            metabolic: MetabolicGate::new(MetabolicLimits::default()),
            token: TokenVerifier::new(),
            generation,
            drone_name: drone_name.to_string(),
        }
    }

    pub fn with_limits(drone_name: &str, generation: u32, limits: MetabolicLimits) -> Self {
        Self {
            metabolic: MetabolicGate::new(limits),
            token: TokenVerifier::new(),
            generation,
            drone_name: drone_name.to_string(),
        }
    }

    /// Full authorization check for replication
    pub fn authorize_replication(&self) -> Result<(), LeashDenial> {
        // 1. Check token
        self.token.check_token()?;

        // 2. Check metabolic limits
        self.metabolic.can_replicate(self.generation)?;

        // 3. Check apoptosis
        if is_apoptosis() {
            return Err(LeashDenial::ApoptosisActive);
        }

        Ok(())
    }

    /// Handle an incoming BioMessage — check for leash commands
    pub fn handle_message(&mut self, msg: &BioMessage) -> Result<(), LeashDenial> {
        // Always check for apoptosis
        if ApoptosisHandler::check_message(msg) {
            return Err(LeashDenial::ApoptosisActive);
        }

        // Check for cryostasis commands
        if let Some(cryo_op) = CryostasisHandler::check_message(msg) {
            match cryo_op {
                BioOp::Freeze => {
                    let _ = CryostasisHandler::execute_freeze(
                        "FREEZE command from Queen",
                        self.generation,
                        vec![self.drone_name.clone()],
                    );
                    return Err(LeashDenial::CryostasisActive);
                }
                BioOp::Thaw => {
                    let _ = CryostasisHandler::execute_thaw(None);
                }
                _ => {}
            }
        }

        match msg.op {
            BioOp::TokenGrant => {
                if let Some(token) = DroneToken::from_bytes(&msg.payload) {
                    eprintln!("[LEASH] Token granted for {}, expires in {}s",
                        self.drone_name, token.expires_at - token.issued_at);
                    self.token.set_token(token);
                }
            }
            BioOp::TokenRevoke => {
                eprintln!("[LEASH] Token revoked for {}", self.drone_name);
                self.token = TokenVerifier::new();
            }
            BioOp::CrisprPatch => {
                CrisprHandler::handle_patch(&mut self.metabolic, &msg.payload);
            }
            BioOp::ImmuneAlert => {
                // Payload: [alert_type:1][severity:1][rest...]
                if msg.payload.len() >= 2 {
                    let alert_type = AlertType::from_byte(msg.payload[0])
                        .unwrap_or(AlertType::AnomalousTraffic);
                    let severity = AlertSeverity::from_byte(msg.payload[1])
                        .unwrap_or(AlertSeverity::Medium);
                    ImmuneHandler::handle_alert(&mut self.metabolic, alert_type, severity);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Status report
    pub fn status(&self) -> LeashStatus {
        LeashStatus {
            drone_name: self.drone_name.clone(),
            generation: self.generation,
            has_token: self.token.has_valid_token(),
            apoptosis: is_apoptosis(),
            cryostasis: is_cryostasis(),
            can_replicate: self.authorize_replication().is_ok(),
            denial_reason: self.authorize_replication().err().map(|e| format!("{}", e)),
        }
    }
}

// ── Denial reasons ──

#[derive(Debug)]
pub enum LeashDenial {
    NoToken,
    TokenExpired,
    GenerationLimit { current: u32, max: u32 },
    CpuOverload { current: f32, limit: f32 },
    MemoryOverload { current: f64, limit: f64 },
    Cooldown { remaining_secs: u64 },
    ApoptosisActive,
    CryostasisActive,
}

impl std::fmt::Display for LeashDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoToken => write!(f, "no metabolic token — Queen authorization required"),
            Self::TokenExpired => write!(f, "token expired — request renewal from Queen"),
            Self::GenerationLimit { current, max } =>
                write!(f, "generation limit: gen {} >= max {}", current, max),
            Self::CpuOverload { current, limit } =>
                write!(f, "CPU overload: {:.1}% > {:.1}% limit", current, limit),
            Self::MemoryOverload { current, limit } =>
                write!(f, "memory overload: {:.1}% > {:.1}% limit", current, limit),
            Self::Cooldown { remaining_secs } =>
                write!(f, "cooldown: {} seconds remaining", remaining_secs),
            Self::ApoptosisActive =>
                write!(f, "APOPTOSIS active — all operations denied"),
            Self::CryostasisActive =>
                write!(f, "CRYOSTASIS active — replication frozen"),
        }
    }
}

// ── Status ──

#[derive(Debug, serde::Serialize)]
pub struct LeashStatus {
    pub drone_name: String,
    pub generation: u32,
    pub has_token: bool,
    pub apoptosis: bool,
    pub cryostasis: bool,
    pub can_replicate: bool,
    pub denial_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bio_protocol;

    #[test]
    fn test_immune_throttle_high_severity() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        assert_eq!(gate.limits.max_cpu_percent, 80.0);
        assert_eq!(gate.limits.cooldown_secs, 30);

        ImmuneHandler::handle_alert(&mut gate, AlertType::IntrusionDetected, AlertSeverity::High);

        // Should halve CPU limit and double cooldown
        assert_eq!(gate.limits.max_cpu_percent, 40.0);
        assert_eq!(gate.limits.cooldown_secs, 60);
    }

    #[test]
    fn test_immune_low_severity_no_throttle() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        ImmuneHandler::handle_alert(&mut gate, AlertType::AnomalousTraffic, AlertSeverity::Low);

        // Low severity: no throttle change
        assert_eq!(gate.limits.max_cpu_percent, 80.0);
        assert_eq!(gate.limits.cooldown_secs, 30);
    }

    #[test]
    fn test_immune_throttle_floor() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        // Repeatedly throttle — should floor at 10%
        for _ in 0..10 {
            ImmuneHandler::handle_alert(&mut gate, AlertType::IntrusionDetected, AlertSeverity::Critical);
        }
        assert!(gate.limits.max_cpu_percent >= 10.0);
        assert!(gate.limits.cooldown_secs <= 300);
    }

    #[test]
    fn test_crispr_patch_max_cpu() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        let payload = bio_protocol::encode_fields(&[
            ("max_cpu", &50.0f32.to_le_bytes() as &[u8]),
        ]);
        CrisprHandler::handle_patch(&mut gate, &payload);
        assert_eq!(gate.limits.max_cpu_percent, 50.0);
    }

    #[test]
    fn test_crispr_patch_cooldown() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        let payload = bio_protocol::encode_fields(&[
            ("cooldown", &120u64.to_le_bytes() as &[u8]),
        ]);
        CrisprHandler::handle_patch(&mut gate, &payload);
        assert_eq!(gate.limits.cooldown_secs, 120);
    }

    #[test]
    fn test_crispr_patch_clamps() {
        let mut gate = MetabolicGate::new(MetabolicLimits::default());
        // Try to set max_cpu to 0 — should clamp to 10
        let payload = bio_protocol::encode_fields(&[
            ("max_cpu", &0.0f32.to_le_bytes() as &[u8]),
        ]);
        CrisprHandler::handle_patch(&mut gate, &payload);
        assert_eq!(gate.limits.max_cpu_percent, 10.0);
    }

    #[test]
    fn test_alert_type_from_byte() {
        assert_eq!(AlertType::from_byte(1), Some(AlertType::IntrusionDetected));
        assert_eq!(AlertType::from_byte(4), Some(AlertType::ResourceExhaustion));
        assert_eq!(AlertType::from_byte(0), None);
        assert_eq!(AlertType::from_byte(5), None);
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Low < AlertSeverity::Medium);
        assert!(AlertSeverity::Medium < AlertSeverity::High);
        assert!(AlertSeverity::High < AlertSeverity::Critical);
    }
}
