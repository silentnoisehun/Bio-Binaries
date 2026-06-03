use crate::bio_client;
use clap::{Parser, Subcommand};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "telepathy-entangle", about = "Shared state via temp files — inter-process state sharing")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set a key-value pair
    Set {
        key: String,
        value: String,
    },
    /// Get a value by key
    Get {
        key: String,
    },
    /// List all entangled states
    List,
    /// Delete a key
    Delete {
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct EntangleEntry {
    pub key: String,
    pub value: Vec<u8>,  // Binary payload (not JSON)
    pub hash: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct EntangleResult {
    pub timestamp: String,
    pub action: String,
    pub entry: Option<EntangleEntry>,
    pub all_entries: Option<Vec<EntangleEntry>>,
    pub success: bool,
}

fn entangle_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("bio-entangle");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn key_path(key: &str) -> std::path::PathBuf {
    entangle_dir().join(format!("{}.bin", key))
}

pub fn set_value(key: &str, value: &str) -> EntangleResult {
    let value_bytes = value.as_bytes().to_vec();

    let entry = EntangleEntry {
        key: key.to_string(),
        value: value_bytes.clone(),
        hash: blake3::hash(&value_bytes).to_hex().to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let path = key_path(key);
    let success = bincode::serialize(&entry)
        .ok()
        .and_then(|data| std::fs::write(&path, data).ok())
        .is_some();

    EntangleResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "SET".into(),
        entry: Some(entry),
        all_entries: None,
        success,
    }
}

pub fn get_value(key: &str) -> EntangleResult {
    let path = key_path(key);
    let entry = std::fs::read(&path)
        .ok()
        .and_then(|data| bincode::deserialize::<EntangleEntry>(&data).ok());

    EntangleResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "GET".into(),
        entry: entry.clone(),
        all_entries: None,
        success: entry.is_some(),
    }
}

pub fn list_all() -> EntangleResult {
    let dir = entangle_dir();
    let mut entries = Vec::new();

    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            if entry.path().extension().map(|e| e == "bin").unwrap_or(false) {
                if let Ok(data) = std::fs::read(entry.path()) {
                    if let Ok(e) = bincode::deserialize::<EntangleEntry>(&data) {
                        entries.push(e);
                    }
                }
            }
        }
    }

    EntangleResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "LIST".into(),
        entry: None,
        all_entries: Some(entries),
        success: true,
    }
}

pub fn delete_key(key: &str) -> EntangleResult {
    let path = key_path(key);
    let success = std::fs::remove_file(&path).is_ok();

    EntangleResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "DELETE".into(),
        entry: None,
        all_entries: None,
        success,
    }
}

fn format_pretty(result: &EntangleResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  🔮 TELEPATHY-ENTANGLE \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / Shared State\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ Action: {}\n", result.action));

    if let Some(ref entry) = result.entry {
        out.push_str(&format!("    Key: {}\n", entry.key));
        let val_str = String::from_utf8_lossy(&entry.value).to_string();
        out.push_str(&format!("    Value: {}\n", val_str));
        out.push_str(&format!("    Hash: {}\n", &entry.hash[..32.min(entry.hash.len())]));
        out.push_str(&format!("    Updated: {}\n", entry.updated_at));
    }

    if let Some(ref entries) = result.all_entries {
        out.push_str(&format!("    Total Entries: {}\n", entries.len()));
        for e in entries {
            let val_str = String::from_utf8_lossy(&e.value).to_string();
            out.push_str(&format!("    {}: {}\n", e.key, val_str));
        }
    }

    let status = if result.success { "SUCCESS" } else { "FAILED" };
    out.push_str(&format!("\n  ⟫ telepathy-entangle :: {}\n\n", status));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let result = match cli.command {
        Commands::Set { key, value } => set_value(&key, &value),
        Commands::Get { key } => get_value(&key),
        Commands::List => list_all(),
        Commands::Delete { key } => delete_key(&key),
    };

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("telepathy-entangle", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
