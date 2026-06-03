use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "eqm-methy", about = "File consolidator — BLAKE3 integrity index with methylation rate")]
pub struct Cli {
    /// Directory to index
    #[arg(default_value = ".")]
    pub path: String,

    /// Importance threshold (0.0 - 1.0): files more recently modified score higher
    #[arg(long, default_value = "0.3")]
    pub threshold: f64,

    /// Maximum depth
    #[arg(long, default_value = "10")]
    pub depth: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MethyFile {
    pub path: String,
    pub blake3_hash: String,
    pub size_bytes: u64,
    pub last_modified: String,
    pub methylation_rate: f64,
    pub important: bool,
}

#[derive(Debug, Serialize)]
pub struct MethyResult {
    pub timestamp: String,
    pub root: String,
    pub total_files: usize,
    pub indexed_files: usize,
    pub important_files: usize,
    pub files: Vec<MethyFile>,
    pub index_file: String,
}

pub fn run(path: &str, threshold: f64, depth: usize) -> MethyResult {
    let now = std::time::SystemTime::now();
    let mut files = Vec::new();
    let mut important_count = 0;

    // Find the oldest and newest modification times for normalization
    let mut oldest_age = 0.0f64;
    let mut entries_data: Vec<(String, u64, std::time::SystemTime)> = Vec::new();

    for entry in WalkDir::new(path).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = meta.modified().unwrap_or(now);
        let age = now.duration_since(modified).unwrap_or_default().as_secs_f64();
        if age > oldest_age { oldest_age = age; }
        entries_data.push((entry.path().to_string_lossy().to_string(), meta.len(), modified));
    }

    if oldest_age == 0.0 { oldest_age = 1.0; }

    for (fpath, size, modified) in &entries_data {
        // BLAKE3 hash
        let hash = match std::fs::read(fpath) {
            Ok(data) => blake3::hash(&data).to_hex().to_string(),
            Err(_) => "error".to_string(),
        };

        let age = now.duration_since(*modified).unwrap_or_default().as_secs_f64();
        // Methylation rate: recently modified = high rate (1.0), old = low rate (0.0)
        let methylation = 1.0 - (age / oldest_age);
        let important = methylation >= threshold;
        if important { important_count += 1; }

        let mod_time = modified.duration_since(std::time::UNIX_EPOCH)
            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default())
            .unwrap_or_default();

        files.push(MethyFile {
            path: fpath.clone(),
            blake3_hash: hash,
            size_bytes: *size,
            last_modified: mod_time,
            methylation_rate: (methylation * 1000.0).round() / 1000.0,
            important,
        });
    }

    files.sort_by(|a, b| b.methylation_rate.partial_cmp(&a.methylation_rate).unwrap_or(std::cmp::Ordering::Equal));

    // Write index file
    let index_path = format!("{}/.methy-index.bin", path.trim_end_matches('/').trim_end_matches('\\'));
    let index_data: Vec<_> = files.iter().filter(|f| f.important).collect();
    if let Ok(serialized) = bincode::serialize(&index_data) {
        let _ = std::fs::write(&index_path, serialized);
    }

    MethyResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: path.to_string(),
        total_files: files.len(),
        indexed_files: files.len(),
        important_files: important_count,
        files,
        index_file: index_path,
    }
}

fn format_pretty(result: &MethyResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  🧬 EQM-METHY \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / File Consolidator\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ Index Summary\n"));
    out.push_str(&format!("    Root: {}\n", result.root));
    out.push_str(&format!("    Total Files: {}\n", result.total_files));
    out.push_str(&format!("    Important (above threshold): {}\n", result.important_files));
    out.push_str(&format!("    Index Written: {}\n", result.index_file));

    out.push_str("\n");
    out.push_str(&format!("  ▸ Top Files by Methylation Rate\n"));
    for f in result.files.iter().take(15) {
        let marker = if f.important { "●" } else { "○" };
        let rate_str = format!("{:.3}", f.methylation_rate);
        let hash_preview = if f.blake3_hash.len() >= 16 { &f.blake3_hash[..16] } else { &f.blake3_hash };
        out.push_str(&format!("    {}: {} [{}] {}\n", marker, f.path, rate_str, hash_preview));
    }

    out.push_str(&format!("\n  ⟫ eqm-methy :: {}/{} files indexed as important\n\n",
        result.important_files, result.total_files));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.path, cli.threshold, cli.depth);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("eqm-methy", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
