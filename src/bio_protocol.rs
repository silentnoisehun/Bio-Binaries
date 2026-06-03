/// Bio-Binary DNS-level Protocol
///
/// Fixed-layout binary message format replacing JSON.
/// Zero-copy, no serialization overhead, cryptographically authenticated.
///
/// Layout (total header: 60 bytes):
///   [magic:2][op:1][flags:1][generation:4][nonce:8][auth_tag:32][payload_len:4][checksum:8]
///   [payload: payload_len bytes]
///
/// The 8-byte nonce (nanosecond timestamp) prevents replay attacks.
/// Receivers maintain a seen-nonce window to reject duplicates.

pub const BIO_MAGIC: [u8; 2] = [0xB1, 0x0B]; // "BIOB" marker
pub const BIO_HEADER_SIZE: usize = 60;
pub const MAX_GENERATION: u32 = 16; // Hard limit on clone depth

// ── Opcodes ──
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BioOp {
    // Original Echo-X opcodes
    Join      = 0x10,
    Task      = 0x11,
    Result    = 0x12,
    Heartbeat = 0x13,
    Status    = 0x14,
    Shutdown  = 0x15,
    // New: Mitosis opcodes
    Clone     = 0x20, // Request to replicate
    Genome    = 0x21, // Binary payload (self-genome transfer)
    GenomeAck = 0x22, // Genome received + spawned
    // New: Leash opcodes
    TokenGrant   = 0x30, // Queen grants metabolic token
    TokenRevoke  = 0x31, // Queen revokes token
    Apoptosis    = 0x3F, // Emergency kill — all drones self-destruct
    // Cryostasis opcodes
    Freeze       = 0x40, // Queen → Drone: freeze state
    Thaw         = 0x41, // Queen → Drone: thaw state
    FrozenAck    = 0x42, // Drone → Queen: frozen (payload: name + hash + size)
    // Acoustic modem opcodes
    AcousticTx   = 0x50, // Acoustic transmission initiated
    AcousticRx   = 0x51, // Acoustic reception confirmed
    // Bio-Core opcodes
    CrisprPatch  = 0x60, // Runtime patch command (config override / throttle adjust)
    ImmuneAlert  = 0x61, // Immune system alert (intrusion, integrity, anomaly, resource)
    // Microscope Memory opcodes
    MicroQuery   = 0x70, // Spatial/zoom query
    MicroResult  = 0x71, // Query results (top-k)
    MicroIngest  = 0x72, // Append memory block
    // Homeostasis opcodes
    HomeoSync    = 0x80, // System-wide homeostasis sync (Queen <-> Drones)
}

impl BioOp {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x10 => Some(Self::Join),
            0x11 => Some(Self::Task),
            0x12 => Some(Self::Result),
            0x13 => Some(Self::Heartbeat),
            0x14 => Some(Self::Status),
            0x15 => Some(Self::Shutdown),
            0x20 => Some(Self::Clone),
            0x21 => Some(Self::Genome),
            0x22 => Some(Self::GenomeAck),
            0x30 => Some(Self::TokenGrant),
            0x31 => Some(Self::TokenRevoke),
            0x3F => Some(Self::Apoptosis),
            0x40 => Some(Self::Freeze),
            0x41 => Some(Self::Thaw),
            0x42 => Some(Self::FrozenAck),
            0x50 => Some(Self::AcousticTx),
            0x51 => Some(Self::AcousticRx),
            0x60 => Some(Self::CrisprPatch),
            0x61 => Some(Self::ImmuneAlert),
            0x70 => Some(Self::MicroQuery),
            0x71 => Some(Self::MicroResult),
            0x72 => Some(Self::MicroIngest),
            0x80 => Some(Self::HomeoSync),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Join => "JOIN",
            Self::Task => "TASK",
            Self::Result => "RESULT",
            Self::Heartbeat => "HEARTBEAT",
            Self::Status => "STATUS",
            Self::Shutdown => "SHUTDOWN",
            Self::Clone => "CLONE",
            Self::Genome => "GENOME",
            Self::GenomeAck => "GENOME_ACK",
            Self::TokenGrant => "TOKEN_GRANT",
            Self::TokenRevoke => "TOKEN_REVOKE",
            Self::Apoptosis => "APOPTOSIS",
            Self::Freeze => "FREEZE",
            Self::Thaw => "THAW",
            Self::FrozenAck => "FROZEN_ACK",
            Self::AcousticTx => "ACOUSTIC_TX",
            Self::AcousticRx => "ACOUSTIC_RX",
            Self::CrisprPatch => "CRISPR_PATCH",
            Self::ImmuneAlert => "IMMUNE_ALERT",
            Self::MicroQuery => "MICRO_QUERY",
            Self::MicroResult => "MICRO_RESULT",
            Self::MicroIngest => "MICRO_INGEST",
            Self::HomeoSync => "HOMEO_SYNC",
        }
    }
}

// ── Flags ──
pub mod flags {
    pub const ENCRYPTED: u8    = 0b0000_0001;
    pub const COMPRESSED: u8   = 0b0000_0010;
    pub const URGENT: u8       = 0b0000_0100;
    pub const QUEEN_ORIGIN: u8 = 0b0000_1000; // Only Queen can set this
    pub const ACK_REQUIRED: u8 = 0b0001_0000;
}

// ── Nonce generation ──
fn generate_nonce() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

// ── CRC64 (ECMA-182) ──
const CRC64_POLY: u64 = 0x42F0E1EBA9EA3693;

fn crc64_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    for i in 0..256u64 {
        let mut crc = i;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ CRC64_POLY;
            } else {
                crc >>= 1;
            }
        }
        table[i as usize] = crc;
    }
    table
}

pub fn crc64(data: &[u8]) -> u64 {
    let table = crc64_table();
    let mut crc: u64 = 0xFFFFFFFFFFFFFFFF;
    for &byte in data {
        let idx = ((crc ^ byte as u64) & 0xFF) as usize;
        crc = (crc >> 8) ^ table[idx];
    }
    crc ^ 0xFFFFFFFFFFFFFFFF
}

// ── BioMessage ──
#[derive(Clone)]
pub struct BioMessage {
    pub op: BioOp,
    pub flags: u8,
    pub generation: u32,
    pub nonce: u64,            // Replay-protection nonce (nanosecond timestamp)
    pub auth_tag: [u8; 32],    // BLAKE3 keyed hash
    pub payload: Vec<u8>,
}

impl std::fmt::Debug for BioMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BioMessage")
            .field("op", &self.op)
            .field("flags", &format!("0b{:08b}", self.flags))
            .field("generation", &self.generation)
            .field("nonce", &self.nonce)
            .field("auth_tag", &hex(&self.auth_tag[..8]))
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

impl BioMessage {
    /// Create a new message (auth_tag filled later by signing, nonce auto-generated)
    pub fn new(op: BioOp, generation: u32, payload: Vec<u8>) -> Self {
        Self {
            op,
            flags: 0,
            generation,
            nonce: generate_nonce(),
            auth_tag: [0u8; 32],
            payload,
        }
    }

    /// Create a message with flags
    pub fn with_flags(op: BioOp, flags: u8, generation: u32, payload: Vec<u8>) -> Self {
        Self {
            op,
            flags,
            generation,
            nonce: generate_nonce(),
            auth_tag: [0u8; 32],
            payload,
        }
    }

    /// Sign the message with a key (BLAKE3 keyed hash over header+payload)
    pub fn sign(&mut self, key: &[u8; 32]) {
        let signable = self.signable_bytes();
        let hash = blake3::keyed_hash(key, &signable);
        self.auth_tag.copy_from_slice(hash.as_bytes());
    }

    /// Verify the auth_tag against a key
    pub fn verify(&self, key: &[u8; 32]) -> bool {
        let signable = self.signable_bytes();
        let hash = blake3::keyed_hash(key, &signable);
        constant_time_eq(&self.auth_tag, hash.as_bytes())
    }

    /// Bytes used for signing (everything except auth_tag itself)
    fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + self.payload.len());
        buf.extend_from_slice(&BIO_MAGIC);
        buf.push(self.op as u8);
        buf.push(self.flags);
        buf.extend_from_slice(&self.generation.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Encode to wire format
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;

        // Build header without checksum first
        let mut buf = Vec::with_capacity(BIO_HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(&BIO_MAGIC);                     // 2
        buf.push(self.op as u8);                                // 1
        buf.push(self.flags);                                   // 1
        buf.extend_from_slice(&self.generation.to_le_bytes());  // 4
        buf.extend_from_slice(&self.nonce.to_le_bytes());       // 8
        buf.extend_from_slice(&self.auth_tag);                  // 32
        buf.extend_from_slice(&payload_len.to_le_bytes());      // 4

        // Checksum covers header (without checksum field) + payload
        let mut checksum_input = buf.clone();
        checksum_input.extend_from_slice(&self.payload);
        let checksum = crc64(&checksum_input);
        buf.extend_from_slice(&checksum.to_le_bytes());         // 8

        // Total header: 2+1+1+4+8+32+4+8 = 60 bytes
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decode from wire format
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < BIO_HEADER_SIZE {
            return Err(ProtocolError::TooShort);
        }

        // Magic check
        if data[0] != BIO_MAGIC[0] || data[1] != BIO_MAGIC[1] {
            return Err(ProtocolError::BadMagic);
        }

        let op = BioOp::from_byte(data[2]).ok_or(ProtocolError::UnknownOpcode(data[2]))?;
        let flags = data[3];
        let generation = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let nonce = u64::from_le_bytes([
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15],
        ]);

        let mut auth_tag = [0u8; 32];
        auth_tag.copy_from_slice(&data[16..48]);

        let payload_len = u32::from_le_bytes([data[48], data[49], data[50], data[51]]) as usize;
        let checksum = u64::from_le_bytes([
            data[52], data[53], data[54], data[55],
            data[56], data[57], data[58], data[59],
        ]);

        // Check total length
        if data.len() < BIO_HEADER_SIZE + payload_len {
            return Err(ProtocolError::PayloadTruncated);
        }

        let payload = data[BIO_HEADER_SIZE..BIO_HEADER_SIZE + payload_len].to_vec();

        // Verify checksum
        let mut checksum_input = data[..52].to_vec(); // header without checksum
        checksum_input.extend_from_slice(&payload);
        let computed = crc64(&checksum_input);
        if computed != checksum {
            return Err(ProtocolError::ChecksumMismatch { expected: checksum, computed });
        }

        Ok(Self { op, flags, generation, nonce, auth_tag, payload })
    }
}

// ── Protocol Errors ──
#[derive(Debug)]
pub enum ProtocolError {
    TooShort,
    BadMagic,
    UnknownOpcode(u8),
    PayloadTruncated,
    ChecksumMismatch { expected: u64, computed: u64 },
    AuthFailed,
    GenerationExceeded { gen: u32, max: u32 },
    TokenExpired,
    ReplayDetected { nonce: u64 },
    Apoptosis,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "message too short"),
            Self::BadMagic => write!(f, "bad magic bytes (not BioMessage)"),
            Self::UnknownOpcode(op) => write!(f, "unknown opcode: 0x{:02X}", op),
            Self::PayloadTruncated => write!(f, "payload truncated"),
            Self::ChecksumMismatch { expected, computed } =>
                write!(f, "CRC64 mismatch: expected={:016X} computed={:016X}", expected, computed),
            Self::AuthFailed => write!(f, "auth tag verification failed"),
            Self::GenerationExceeded { gen, max } =>
                write!(f, "generation {} exceeds max {}", gen, max),
            Self::TokenExpired => write!(f, "metabolic token expired or revoked"),
            Self::ReplayDetected { nonce } =>
                write!(f, "replay detected: nonce {} already seen", nonce),
            Self::Apoptosis => write!(f, "APOPTOSIS signal received — terminating"),
        }
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

pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── Payload helpers (raw binary, no JSON) ──

/// Encode a simple key-value pair into binary payload
/// Format: [key_len:2][key][value_len:4][value]
pub fn encode_kv(key: &str, value: &[u8]) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    let mut buf = Vec::with_capacity(6 + key_bytes.len() + value.len());
    buf.extend_from_slice(&(key_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(key_bytes);
    buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
    buf.extend_from_slice(value);
    buf
}

/// Decode a key-value pair from binary payload
pub fn decode_kv(data: &[u8]) -> Option<(&str, &[u8])> {
    if data.len() < 6 { return None; }
    let key_len = u16::from_le_bytes([data[0], data[1]]) as usize;
    if data.len() < 6 + key_len { return None; }
    let key = std::str::from_utf8(&data[2..2 + key_len]).ok()?;
    let val_len = u32::from_le_bytes([
        data[2 + key_len], data[3 + key_len],
        data[4 + key_len], data[5 + key_len],
    ]) as usize;
    let val_start = 6 + key_len;
    if data.len() < val_start + val_len { return None; }
    Some((key, &data[val_start..val_start + val_len]))
}

/// Encode multiple fields into binary payload
/// Format: [field_count:2] [field1] [field2] ...
/// Each field: [key_len:2][key][value_len:4][value]
pub fn encode_fields(fields: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(fields.len() as u16).to_le_bytes());
    for (key, value) in fields {
        buf.extend_from_slice(&encode_kv(key, value));
    }
    buf
}

/// Decode multiple fields from binary payload
pub fn decode_fields(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    if data.len() < 2 { return vec![]; }
    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut results = Vec::with_capacity(count);
    let mut offset = 2;
    for _ in 0..count {
        if offset >= data.len() { break; }
        if let Some((key, value)) = decode_kv(&data[offset..]) {
            let consumed = 6 + key.len() + value.len();
            results.push((key.to_string(), value.to_vec()));
            offset += consumed;
        } else {
            break;
        }
    }
    results
}

// ── Replay protection (seen-nonce window) ──

/// Rolling nonce window for replay detection.
/// Maintains a bounded set of recently-seen nonces.
pub struct NonceWindow {
    seen: std::collections::HashSet<u64>,
    order: std::collections::VecDeque<u64>,
    max_size: usize,
}

impl NonceWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            seen: std::collections::HashSet::with_capacity(max_size),
            order: std::collections::VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Returns true if nonce is fresh (not seen before).
    /// Returns false if replay detected.
    pub fn check_and_insert(&mut self, nonce: u64) -> bool {
        if self.seen.contains(&nonce) {
            return false;
        }
        self.seen.insert(nonce);
        self.order.push_back(nonce);
        while self.order.len() > self.max_size {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let key = [0x42u8; 32];
        let mut msg = BioMessage::new(BioOp::Heartbeat, 3, b"hello drone".to_vec());
        msg.sign(&key);

        let encoded = msg.encode();
        let decoded = BioMessage::decode(&encoded).unwrap();

        assert_eq!(decoded.op, BioOp::Heartbeat);
        assert_eq!(decoded.generation, 3);
        assert_eq!(decoded.payload, b"hello drone");
        assert!(decoded.verify(&key));
    }

    #[test]
    fn test_bad_checksum_rejected() {
        let mut msg = BioMessage::new(BioOp::Task, 0, b"test".to_vec());
        let mut encoded = msg.encode();
        // Corrupt a payload byte
        if let Some(last) = encoded.last_mut() {
            *last ^= 0xFF;
        }
        assert!(BioMessage::decode(&encoded).is_err());
    }

    #[test]
    fn test_bad_auth_rejected() {
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let mut msg = BioMessage::new(BioOp::Task, 0, b"test".to_vec());
        msg.sign(&key1);
        assert!(!msg.verify(&key2));
    }

    #[test]
    fn test_fields_roundtrip() {
        let pid_bytes = 42u32.to_le_bytes();
        let fields = vec![
            ("name", b"drone-01" as &[u8]),
            ("pid", &pid_bytes as &[u8]),
        ];
        let encoded = encode_fields(&fields);
        let decoded = decode_fields(&encoded);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, "name");
        assert_eq!(decoded[0].1, b"drone-01");
        assert_eq!(decoded[1].0, "pid");
    }

    #[test]
    fn test_nonce_uniqueness() {
        let msg1 = BioMessage::new(BioOp::Heartbeat, 0, vec![]);
        let msg2 = BioMessage::new(BioOp::Heartbeat, 0, vec![]);
        // Nonces should differ (nanosecond timestamps)
        assert_ne!(msg1.nonce, msg2.nonce);
    }

    #[test]
    fn test_nonce_survives_roundtrip() {
        let key = [0x42u8; 32];
        let mut msg = BioMessage::new(BioOp::Task, 1, b"data".to_vec());
        let original_nonce = msg.nonce;
        msg.sign(&key);
        let encoded = msg.encode();
        let decoded = BioMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.nonce, original_nonce);
        assert!(decoded.verify(&key));
    }

    #[test]
    fn test_nonce_window_rejects_replay() {
        let mut window = NonceWindow::new(100);
        assert!(window.check_and_insert(42));  // First time: fresh
        assert!(!window.check_and_insert(42)); // Second time: replay
        assert!(window.check_and_insert(43));  // Different nonce: fresh
    }

    #[test]
    fn test_nonce_window_eviction() {
        let mut window = NonceWindow::new(3);
        assert!(window.check_and_insert(1));
        assert!(window.check_and_insert(2));
        assert!(window.check_and_insert(3));
        // Window full, oldest (1) should be evicted
        assert!(window.check_and_insert(4));
        // Nonce 1 was evicted, so it's accepted again
        assert!(window.check_and_insert(1));
    }

    #[test]
    fn test_header_size_is_60() {
        let msg = BioMessage::new(BioOp::Heartbeat, 0, vec![]);
        let encoded = msg.encode();
        assert_eq!(encoded.len(), BIO_HEADER_SIZE);
        assert_eq!(BIO_HEADER_SIZE, 60);
    }
}
