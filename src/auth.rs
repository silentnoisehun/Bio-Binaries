/// Authentication & Authorization
///
/// BLAKE3 keyed hash for message signing.
/// Queen master key + drone session tokens.
/// Startup binary integrity verification.

use crate::bio_protocol::BioMessage;
use std::path::Path;

const KEY_FILE: &str = ".bio-queen.key";

/// 32-byte master key (Queen only)
#[derive(Clone)]
pub struct QueenKey {
    key: [u8; 32],
}

impl QueenKey {
    /// Generate a new random key
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        // Use BLAKE3 hash of high-entropy sources
        let entropy = format!(
            "{}:{}:{}:{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::env::current_exe().unwrap_or_default().display(),
            rand_bytes(),
        );
        let hash = blake3::hash(entropy.as_bytes());
        key.copy_from_slice(hash.as_bytes());
        Self { key }
    }

    /// Load key from file, or generate and save
    pub fn load_or_create(dir: &str) -> std::io::Result<Self> {
        let path = Path::new(dir).join(KEY_FILE);
        if path.exists() {
            let data = std::fs::read(&path)?;
            if data.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&data);
                return Ok(Self { key });
            }
        }
        let queen = Self::generate();
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, &queen.key)?;
        Ok(queen)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    /// Sign a BioMessage
    pub fn sign(&self, msg: &mut BioMessage) {
        msg.sign(&self.key);
    }

    /// Verify a BioMessage
    pub fn verify(&self, msg: &BioMessage) -> bool {
        msg.verify(&self.key)
    }

    /// Generate a session token for a drone
    pub fn issue_token(&self, drone_name: &str, generation: u32) -> DroneToken {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires = now + 3600; // 1 hour

        let token_data = format!("{}:{}:{}", drone_name, generation, expires);
        let hash = blake3::keyed_hash(&self.key, token_data.as_bytes());
        let mut signature = [0u8; 32];
        signature.copy_from_slice(hash.as_bytes());

        DroneToken {
            drone_name: drone_name.to_string(),
            generation,
            issued_at: now,
            expires_at: expires,
            signature,
        }
    }
}

/// Session token for a drone
#[derive(Clone, Debug)]
pub struct DroneToken {
    pub drone_name: String,
    pub generation: u32,
    pub issued_at: u64,
    pub expires_at: u64,
    pub signature: [u8; 32],
}

impl DroneToken {
    /// Encode token to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let name_bytes = self.drone_name.as_bytes();
        let mut buf = Vec::with_capacity(80 + name_bytes.len());
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&self.generation.to_le_bytes());
        buf.extend_from_slice(&self.issued_at.to_le_bytes());
        buf.extend_from_slice(&self.expires_at.to_le_bytes());
        buf.extend_from_slice(&self.signature);
        buf
    }

    /// Decode token from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 54 { return None; }
        let name_len = u16::from_le_bytes([data[0], data[1]]) as usize;
        if data.len() < 54 + name_len { return None; }
        let drone_name = std::str::from_utf8(&data[2..2 + name_len]).ok()?.to_string();
        let off = 2 + name_len;
        let generation = u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]);
        let issued_at = u64::from_le_bytes([
            data[off+4], data[off+5], data[off+6], data[off+7],
            data[off+8], data[off+9], data[off+10], data[off+11],
        ]);
        let expires_at = u64::from_le_bytes([
            data[off+12], data[off+13], data[off+14], data[off+15],
            data[off+16], data[off+17], data[off+18], data[off+19],
        ]);
        let mut signature = [0u8; 32];
        signature.copy_from_slice(&data[off+20..off+52]);
        Some(Self { drone_name, generation, issued_at, expires_at, signature })
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= self.expires_at
    }

    /// Verify token against queen key
    pub fn verify(&self, queen_key: &QueenKey) -> bool {
        let token_data = format!("{}:{}:{}", self.drone_name, self.generation, self.expires_at);
        let hash = blake3::keyed_hash(queen_key.as_bytes(), token_data.as_bytes());
        let mut expected = [0u8; 32];
        expected.copy_from_slice(hash.as_bytes());
        constant_time_eq(&self.signature, &expected)
    }
}

/// Binary self-integrity check — verify the running binary matches its stored BLAKE3 hash.
/// On first run, stores the hash. On subsequent runs, compares against the stored hash.
/// This detects changes to the binary (e.g., recompilation) — not external tampering.
pub struct BinaryIntegrity;

impl BinaryIntegrity {
    /// Hash the current running binary
    pub fn self_hash() -> Option<String> {
        let exe_path = std::env::current_exe().ok()?;
        let data = std::fs::read(&exe_path).ok()?;
        Some(blake3::hash(&data).to_hex().to_string())
    }

    /// Verify against a known hash
    pub fn verify_self(expected_hash: &str) -> bool {
        match Self::self_hash() {
            Some(actual) => actual == expected_hash,
            None => false,
        }
    }

    /// Save current binary hash to a sidecar file
    pub fn save_hash(dir: &str) -> std::io::Result<String> {
        let hash = Self::self_hash()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "cannot hash self"))?;
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let hash_path = Path::new(dir).join(format!("{}.blake3", exe_name));
        std::fs::create_dir_all(dir)?;
        std::fs::write(&hash_path, &hash)?;
        Ok(hash)
    }

    /// Check current binary against saved hash
    pub fn check_integrity(dir: &str) -> Result<bool, std::io::Error> {
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let hash_path = Path::new(dir).join(format!("{}.blake3", exe_name));

        if !hash_path.exists() {
            // No saved hash — first run, save it
            Self::save_hash(dir)?;
            return Ok(true);
        }

        let expected = std::fs::read_to_string(&hash_path)?;
        Ok(Self::verify_self(expected.trim()))
    }
}

// ── Helpers ──

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn rand_bytes() -> String {
    // Poor man's entropy — use multiple time sources + address space
    let a = std::time::Instant::now();
    let b: usize = &a as *const _ as usize;
    let c = std::thread::current().id();
    format!("{:?}:{:x}:{:?}", a, b, c)
}

/// Macro for drone startup self-integrity gate.
/// Compares own BLAKE3 hash against a stored sidecar file.
/// Exits with code 77 if the binary has changed since last verified run.
/// This is a self-integrity check, not tamper-proofing.
#[macro_export]
macro_rules! bio_auth_gate {
    ($name:expr) => {{
        let integrity_dir = std::env::temp_dir()
            .join("bio-integrity")
            .to_string_lossy()
            .to_string();
        match $crate::auth::BinaryIntegrity::check_integrity(&integrity_dir) {
            Ok(true) => { /* OK — binary intact */ }
            Ok(false) => {
                eprintln!("[BIO-SECURITY] Binary integrity check FAILED for {}.", $name);
                eprintln!("[BIO-SECURITY] The executable has been modified since last verified run.");
                eprintln!("[BIO-SECURITY] Possible mutation detected (binary changed since last run). Aborting.");
                std::process::exit(77);
            }
            Err(e) => {
                eprintln!("[BIO-SECURITY] Cannot verify binary integrity: {}. Proceeding cautiously.", e);
            }
        }
    }};
}
