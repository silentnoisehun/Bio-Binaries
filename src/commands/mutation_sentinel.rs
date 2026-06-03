/// Mutation Sentinel — watches .rs files for changes and triggers auto-freeze
///
/// Monitors a directory recursively for Rust source file modifications.
/// On change: computes BLAKE3 hash, optionally triggers cryo::freeze(),
/// and optionally sends BioOp::Freeze via Echo-X UDP.

use crate::output;
use crate::wave_store::{self, WavePacket, WaveOrigin, channels};
use clap::{Parser, Subcommand};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::Path;
use std::sync::mpsc;

#[derive(Parser)]
#[command(name = "mutation-sentinel", about = "File mutation watcher — auto-freeze on .rs changes")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Monitor a directory for .rs file changes
    Watch {
        /// Directory to monitor
        #[arg(default_value = ".")]
        path: String,

        /// Auto-freeze on change
        #[arg(long)]
        auto_freeze: bool,
    },
    /// Compute hash of a file
    Hash {
        /// File path
        #[arg()]
        file: String,
    },
}

#[derive(Debug, Serialize)]
pub struct MutationEvent {
    pub timestamp: String,
    pub file: String,
    pub event_type: String,
    pub hash: String,
    pub frozen: bool,
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    match cli.command {
        Commands::Watch { path, auto_freeze } => {
            output::banner("MUTATION-SENTINEL", "File Mutation Watcher", "◈");
            output::section("Watching");
            output::kv("Directory", &path);
            output::kv("Auto-Freeze", if auto_freeze { "enabled" } else { "disabled" });

            eprintln!("[MUTATION-SENTINEL] Starting watcher on {}", path);

            let (tx, rx) = mpsc::channel();
            let mut watcher = RecommendedWatcher::new(tx, Config::default())
                .map_err(|e| e.to_string())?;

            watcher.watch(Path::new(&path), RecursiveMode::Recursive)
                .map_err(|e| e.to_string())?;

            eprintln!("[MUTATION-SENTINEL] Watcher active. Press Ctrl+C to stop.");
            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        if let EventKind::Modify(_) = event.kind {
                            for path in event.paths {
                                if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                                    // Compute BLAKE3 hash of mutated file
                                    let hash_hex = match std::fs::read(&path) {
                                        Ok(data) => blake3::hash(&data).to_hex().to_string(),
                                        Err(_) => "read-error".to_string(),
                                    };

                                    eprintln!("[MUTATION-SENTINEL] Mutation detected: {}", path.display());
                                    eprintln!("[MUTATION-SENTINEL] BLAKE3: {}", &hash_hex[..16]);

                                    // ── SYNAPSE: Inject 28Hz MUTATION wave into WaveField ──
                                    let wave = WavePacket {
                                        emitted_at: wave_store::now_ms(),
                                        frequency: channels::MUTATION,  // 28.0 Hz
                                        amplitude: 0.8,                 // strong signal
                                        decay: 0.0005,                  // slow decay (~2sec half-life)
                                        phase: 0.0,
                                        origin: WaveOrigin::MutationSentinel,
                                        tag: Some(format!("{}|{}", path.display(), &hash_hex[..16])),
                                        ..Default::default()
                                    };
                                    let store_path = wave_store::default_path();
                                    match wave_store::WaveStore::append_to_inbox(&store_path, &wave) {
                                        Ok(_) => eprintln!("[MUTATION-SENTINEL] → Wave injected: 28Hz amp=0.8 → field inbox"),
                                        Err(e) => eprintln!("[MUTATION-SENTINEL] ⚠ Inbox write failed: {}", e),
                                    }

                                    if auto_freeze {
                                        // Also inject SECURITY wave for freeze consideration
                                        let sec_wave = WavePacket {
                                            emitted_at: wave_store::now_ms(),
                                            frequency: channels::SECURITY,  // 60.0 Hz
                                            amplitude: 0.6,
                                            decay: 0.001,
                                            phase: 0.0,
                                            origin: WaveOrigin::MutationSentinel,
                                            tag: Some("auto-freeze-trigger".to_string()),
                                            ..Default::default()
                                        };
                                        let _ = wave_store::WaveStore::append_to_inbox(&store_path, &sec_wave);
                                        eprintln!("[MUTATION-SENTINEL] → Security wave injected: 60Hz → freeze consideration");
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => eprintln!("[MUTATION-SENTINEL] Error: {}", e),
                    Err(_) => break,
                }
            }

            Ok("".to_string())
        }
        Commands::Hash { file } => {
            output::banner("MUTATION-SENTINEL", "File Hash Computation", "◈");
            output::section("Hashing");
            output::kv("File", &file);

            if !Path::new(&file).exists() {
                return Err(format!("File not found: {}", file));
            }

            let contents = std::fs::read(&file).map_err(|e| e.to_string())?;
            let hash = blake3::hash(&contents);
            output::kv("BLAKE3", &hash.to_hex().to_string());
            output::success("Hash computed");
            output::summary("mutation-sentinel", "Hash complete");

            Ok("".to_string())
        }
    }
}
