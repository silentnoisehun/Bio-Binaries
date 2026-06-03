/// Mitosis — Self-replication & Ribosome Code Generation
///
/// A binary can:
/// 1. Read its own genome (executable bytes)
/// 2. Send it over the network to another host
/// 3. The receiver saves and spawns the new instance
///
/// The Ribosome can generate new binaries from templates.

use crate::bio_protocol::{self, BioMessage, BioOp, flags};
use crate::auth::QueenKey;
use std::path::{Path, PathBuf};
use tokio::net::UdpSocket;
use std::net::SocketAddr;

pub const GENOME_CHUNK_SIZE: usize = 32768; // 32KB chunks for UDP transfer
pub const MAX_BINARY_SIZE: usize = 50 * 1024 * 1024; // 50MB hard limit

/// Self-genome reader — reads the current running binary
pub struct Genome;

impl Genome {
    /// Read the current executable's bytes
    pub fn read_self() -> std::io::Result<Vec<u8>> {
        let exe_path = std::env::current_exe()?;
        let data = std::fs::read(&exe_path)?;
        if data.len() > MAX_BINARY_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("binary too large: {} bytes (max {})", data.len(), MAX_BINARY_SIZE),
            ));
        }
        Ok(data)
    }

    /// Get self hash (BLAKE3)
    pub fn self_hash() -> std::io::Result<String> {
        let data = Self::read_self()?;
        Ok(blake3::hash(&data).to_hex().to_string())
    }

    /// Get self path
    pub fn self_path() -> std::io::Result<PathBuf> {
        std::env::current_exe()
    }

    /// Get self name
    pub fn self_name() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Mitosis — replicate self to a target
pub struct Mitosis {
    pub generation: u32,
    queen_key: QueenKey,
}

impl Mitosis {
    pub fn new(generation: u32, queen_key: QueenKey) -> Self {
        Self { generation, queen_key }
    }

    /// Replicate self to a local path (cell division on same host)
    pub fn replicate_local(&self, target_dir: &str, new_name: Option<&str>) -> std::io::Result<ReplicationResult> {
        // Generation limit check
        if self.generation >= bio_protocol::MAX_GENERATION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("generation limit reached: {} >= {}", self.generation, bio_protocol::MAX_GENERATION),
            ));
        }

        let genome = Genome::read_self()?;
        let source_hash = blake3::hash(&genome).to_hex().to_string();
        let source_name = Genome::self_name();

        let target_name = new_name.unwrap_or(&source_name);
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let target_path = Path::new(target_dir).join(format!("{}{}", target_name, ext));

        std::fs::create_dir_all(target_dir)?;
        std::fs::write(&target_path, &genome)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&target_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&target_path, perms)?;
        }

        let clone_hash = blake3::hash(&std::fs::read(&target_path)?).to_hex().to_string();

        let integrity_match = source_hash == clone_hash;
        Ok(ReplicationResult {
            source_name,
            source_hash,
            target_path: target_path.to_string_lossy().to_string(),
            clone_hash,
            generation: self.generation + 1,
            integrity_match,
            bytes_transferred: genome.len(),
        })
    }

    /// Replicate self to a remote host via UDP (chunked transfer)
    pub async fn replicate_remote(
        &self,
        socket: &UdpSocket,
        target_addr: SocketAddr,
    ) -> std::io::Result<ReplicationResult> {
        if self.generation >= bio_protocol::MAX_GENERATION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "generation limit reached",
            ));
        }

        let genome = Genome::read_self()?;
        let source_hash = blake3::hash(&genome).to_hex().to_string();
        let source_name = Genome::self_name();
        let total_chunks = (genome.len() + GENOME_CHUNK_SIZE - 1) / GENOME_CHUNK_SIZE;

        // Send CLONE request first
        let clone_payload = bio_protocol::encode_fields(&[
            ("name", source_name.as_bytes()),
            ("hash", source_hash.as_bytes()),
            ("size", &(genome.len() as u64).to_le_bytes()),
            ("chunks", &(total_chunks as u32).to_le_bytes()),
            ("gen", &(self.generation + 1).to_le_bytes()),
        ]);
        let mut clone_msg = BioMessage::with_flags(
            BioOp::Clone,
            flags::QUEEN_ORIGIN,
            self.generation,
            clone_payload,
        );
        self.queen_key.sign(&mut clone_msg);
        socket.send_to(&clone_msg.encode(), target_addr).await?;

        // Send genome chunks
        for (i, chunk) in genome.chunks(GENOME_CHUNK_SIZE).enumerate() {
            let mut chunk_payload = Vec::with_capacity(8 + chunk.len());
            chunk_payload.extend_from_slice(&(i as u32).to_le_bytes());
            chunk_payload.extend_from_slice(&(total_chunks as u32).to_le_bytes());
            chunk_payload.extend_from_slice(chunk);

            let mut genome_msg = BioMessage::new(
                BioOp::Genome,
                self.generation,
                chunk_payload,
            );
            self.queen_key.sign(&mut genome_msg);
            socket.send_to(&genome_msg.encode(), target_addr).await?;

            // Small delay to avoid overwhelming UDP buffer
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }

        Ok(ReplicationResult {
            source_name,
            source_hash: source_hash.clone(),
            target_path: format!("{}@remote", target_addr),
            clone_hash: source_hash, // Assumed match; receiver will verify
            generation: self.generation + 1,
            integrity_match: true,
            bytes_transferred: genome.len(),
        })
    }
}

/// Result of a replication operation
#[derive(Debug)]
pub struct ReplicationResult {
    pub source_name: String,
    pub source_hash: String,
    pub target_path: String,
    pub clone_hash: String,
    pub generation: u32,
    pub integrity_match: bool,
    pub bytes_transferred: usize,
}

impl serde::Serialize for ReplicationResult {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = s.serialize_struct("ReplicationResult", 7)?;
        state.serialize_field("source_name", &self.source_name)?;
        state.serialize_field("source_hash", &self.source_hash)?;
        state.serialize_field("target_path", &self.target_path)?;
        state.serialize_field("clone_hash", &self.clone_hash)?;
        state.serialize_field("generation", &self.generation)?;
        state.serialize_field("integrity_match", &self.integrity_match)?;
        state.serialize_field("bytes_transferred", &self.bytes_transferred)?;
        state.end()
    }
}

/// Genome receiver — handles incoming CLONE + GENOME messages
pub struct GenomeReceiver {
    pub name: String,
    pub expected_size: usize,
    pub expected_chunks: usize,
    pub expected_hash: String,
    pub generation: u32,
    chunks: Vec<Option<Vec<u8>>>,
    received: usize,
}

impl GenomeReceiver {
    pub fn new(name: String, size: usize, chunks: usize, hash: String, generation: u32) -> Self {
        Self {
            name,
            expected_size: size,
            expected_chunks: chunks,
            expected_hash: hash,
            generation,
            chunks: vec![None; chunks],
            received: 0,
        }
    }

    /// Add a received chunk
    pub fn add_chunk(&mut self, index: usize, data: Vec<u8>) -> bool {
        if index < self.chunks.len() && self.chunks[index].is_none() {
            self.chunks[index] = Some(data);
            self.received += 1;
        }
        self.is_complete()
    }

    pub fn is_complete(&self) -> bool {
        self.received >= self.expected_chunks
    }

    /// Assemble the full genome and verify
    pub fn assemble(&self) -> Result<Vec<u8>, String> {
        if !self.is_complete() {
            return Err(format!("incomplete: {}/{} chunks", self.received, self.expected_chunks));
        }
        let mut genome = Vec::with_capacity(self.expected_size);
        for (i, chunk) in self.chunks.iter().enumerate() {
            match chunk {
                Some(data) => genome.extend_from_slice(data),
                None => return Err(format!("missing chunk {}", i)),
            }
        }

        // Verify hash
        let actual_hash = blake3::hash(&genome).to_hex().to_string();
        if actual_hash != self.expected_hash {
            return Err(format!("hash mismatch: expected {} got {}", self.expected_hash, actual_hash));
        }

        Ok(genome)
    }

    /// Save assembled genome and spawn
    pub fn save_and_spawn(&self, target_dir: &str) -> Result<u32, String> {
        let genome = self.assemble()?;
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let target_path = Path::new(target_dir).join(format!("{}-gen{}{}", self.name, self.generation, ext));

        std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
        std::fs::write(&target_path, &genome).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&target_path).map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&target_path, perms).map_err(|e| e.to_string())?;
        }

        // Spawn the new process
        let child = std::process::Command::new(&target_path)
            .spawn()
            .map_err(|e| e.to_string())?;

        Ok(child.id())
    }
}

/// Ribosome — code generation from templates
pub struct Ribosome;

impl Ribosome {
    /// Generate a new Rust source file from a template
    pub fn synthesize(
        template: &str,
        substitutions: &[(&str, &str)],
        output_path: &str,
    ) -> std::io::Result<()> {
        let mut source = template.to_string();
        for (placeholder, value) in substitutions {
            source = source.replace(placeholder, value);
        }
        std::fs::write(output_path, &source)
    }

    /// Compile a generated source file
    pub fn compile(source_path: &str, output_path: &str) -> std::io::Result<CompileResult> {
        let start = std::time::Instant::now();
        let output = std::process::Command::new("rustc")
            .args([
                "--edition", "2021",
                "-O",
                "-o", output_path,
                source_path,
            ])
            .output()?;

        let duration = start.elapsed();
        let success = output.status.success();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(CompileResult {
            success,
            output_path: output_path.to_string(),
            compile_time_ms: duration.as_millis() as u64,
            errors: if success { None } else { Some(stderr) },
        })
    }

    /// Default drone template
    pub fn drone_template() -> &'static str {
        r#"// Auto-generated Bio-Drone — Generation {{GENERATION}}
// Parent: {{PARENT_NAME}}
// Hash: {{PARENT_HASH}}

use std::net::UdpSocket;

const QUEEN_ADDR: &str = "{{QUEEN_ADDR}}";
const DRONE_NAME: &str = "{{DRONE_NAME}}";
const GENERATION: u32 = {{GENERATION}};

fn main() {
    eprintln!("[{}] Gen-{} drone starting...", DRONE_NAME, GENERATION);

    // Connect to Queen
    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind failed");

    // Send JOIN
    let join_msg = format!("JOIN:{}:{}", DRONE_NAME, GENERATION);
    let _ = socket.send_to(join_msg.as_bytes(), QUEEN_ADDR);

    // Main loop — wait for tasks
    let mut buf = [0u8; 65535];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                let msg = &buf[..len];
                if msg.starts_with(b"SHUTDOWN") || msg.starts_with(b"APOPTOSIS") {
                    eprintln!("[{}] Received shutdown signal. Terminating.", DRONE_NAME);
                    // Self-cleanup
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::fs::remove_file(&exe);
                    }
                    break;
                }
                eprintln!("[{}] Received {} bytes", DRONE_NAME, len);
            }
            Err(e) => {
                eprintln!("[{}] Error: {}", DRONE_NAME, e);
                break;
            }
        }
    }
}
"#
    }
}

/// Result of a compilation
#[derive(Debug, serde::Serialize)]
pub struct CompileResult {
    pub success: bool,
    pub output_path: String,
    pub compile_time_ms: u64,
    pub errors: Option<String>,
}
