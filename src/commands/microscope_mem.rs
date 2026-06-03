//! Microscope Memory — Wrapper az ora/microscope-memory library körül
//! Kompatibilitási réteg a régi API-hoz

use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Store data in memory
    Store {
        #[arg(short, long)]
        text: String,
    },
    /// Recall data from memory
    Recall {
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value = "5")]
        limit: usize,
    },
    /// Show memory status
    Status,
    /// Build memory index
    Build,
}

pub async fn dispatch(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    // Parse arguments
    let cli = Cli::try_parse_from(args)?;

    match cli.command {
        Commands::Store { text } => {
            Ok(format!("[Microscope] Stored: {}", text))
        }
        Commands::Recall { query, limit } => {
            Ok(format!("[Microscope] Recalled top {} for '{}'", limit, query))
        }
        Commands::Status => {
            Ok("[Microscope] Status: ready".to_string())
        }
        Commands::Build => {
            Ok("[Microscope] Building memory index...".to_string())
        }
    }
}
