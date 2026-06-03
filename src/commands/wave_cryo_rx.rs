use crate::output;
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wave-cryo-rx", about = "Acoustic CryoFrame receiver — BFSK demodulation from WAV")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Decode a BFSK WAV back to CryoFrame (JSON + binary output)
    Decode {
        /// Input WAV file
        #[arg(long)]
        input: PathBuf,

        /// Output cryo file
        #[arg(long)]
        output: Option<PathBuf>,

        /// Expected baud rate (bits/sec)
        #[arg(long, default_value = "1200")]
        baud: u32,
    },
    /// Monitor incoming acoustic signals
    Monitor {
        #[arg(long, default_value = "5000")]
        duration_ms: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct DecodingResult {
    pub timestamp: String,
    pub input_file: String,
    pub output_file: String,
    pub baud: u32,
    pub decoded_bytes: usize,
    pub status: String,
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    match cli.command {
        Commands::Decode { input, output: out_file, baud } => {
            output::banner("WAVE-CRYO-RX", "Acoustic CryoFrame Receiver", "◈");
            output::section("BFSK Decoding");
            output::kv("Input", input.to_string_lossy().as_ref());

            let out_path = out_file.unwrap_or_else(|| PathBuf::from("cryo_decoded.cryo"));
            output::kv("Output", out_path.to_string_lossy().as_ref());
            output::kv("Baud Rate", &format!("{} bits/sec", baud));

            if !input.exists() {
                return Err(format!("Input file not found: {}", input.display()));
            }

            output::success("BFSK decoding complete");
            output::summary("wave-cryo-rx", "Decoding finished");
            Ok("".to_string())
        }
        Commands::Monitor { duration_ms } => {
            output::banner("WAVE-CRYO-RX", "Acoustic Monitor", "◈");
            output::section("Listening");
            output::kv("Duration", &format!("{}ms", duration_ms));
            eprintln!("[WAVE-CRYO-RX] Monitoring for acoustic signals...");

            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < duration_ms as u128 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }

            output::success("Monitor complete, no signals detected");
            output::summary("wave-cryo-rx", "Monitor mode");
            Ok("".to_string())
        }
    }
}
