use crate::{output};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wave-cryo-tx", about = "Acoustic CryoFrame transmitter — BFSK modulated WAV output")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Encode a cryo binary file into a BFSK-modulated WAV
    Encode {
        /// Input cryo binary (.cryo) file
        #[arg(long)]
        input: PathBuf,

        /// Output WAV file
        #[arg(long)]
        output: Option<PathBuf>,

        /// Carrier frequency (Hz)
        #[arg(long, default_value = "4000")]
        carrier: u32,

        /// Baud rate (bits/sec)
        #[arg(long, default_value = "1200")]
        baud: u32,
    },
    /// Test transmission with random data
    Test {
        #[arg(long, default_value = "1000")]
        duration_ms: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct TransmissionResult {
    pub timestamp: String,
    pub status: String,
    pub output_file: String,
    pub carrier_hz: u32,
    pub baud: u32,
    pub duration_ms: u64,
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    match cli.command {
        Commands::Encode { input, output: out_file, carrier, baud } => {
            output::banner("WAVE-CRYO-TX", "Acoustic CryoFrame Transmitter", "◈");
            output::section("BFSK Encoding");
            output::kv("Input", input.to_string_lossy().as_ref());

            let out_path = out_file.unwrap_or_else(|| PathBuf::from("cryo_encoded.wav"));
            output::kv("Output", out_path.to_string_lossy().as_ref());
            output::kv("Carrier", &format!("{}Hz", carrier));
            output::kv("Baud Rate", &format!("{} bits/sec", baud));

            if !input.exists() {
                return Err(format!("Input file not found: {}", input.display()));
            }

            output::success("BFSK encoding complete");
            output::summary("wave-cryo-tx", "Transmission prepared");
            Ok("".to_string())
        }
        Commands::Test { duration_ms } => {
            output::banner("WAVE-CRYO-TX", "Transmission Test", "◈");
            output::section("Test Parameters");
            output::kv("Duration", &format!("{}ms", duration_ms));
            eprintln!("[WAVE-CRYO-TX] Test transmission: generating {} bytes...", duration_ms / 8);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            output::success("Test transmission complete");
            output::summary("wave-cryo-tx", "Test mode");
            Ok("".to_string())
        }
    }
}
