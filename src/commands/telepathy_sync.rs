use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "telepathy-sync", about = "Directory synchronizer — BLAKE3 delta sync")]
pub struct Cli {
    /// Source directory
    pub source: String,

    /// Target directory
    pub target: String,

    /// Dry run (don't copy, just report)
    #[arg(long)]
    pub dry_run: bool,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncAction {
    pub path: String,
    pub action: String,
    pub direction: String,
    pub size_bytes: u64,
    pub source_hash: String,
    pub target_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub timestamp: String,
    pub source: String,
    pub target: String,
    pub dry_run: bool,
    pub total_source_files: usize,
    pub total_target_files: usize,
    pub actions: Vec<SyncAction>,
    pub copied: usize,
    pub updated: usize,
    pub skipped: usize,
    pub total_bytes_synced: u64,
}

fn hash_file(path: &std::path::Path) -> Option<String> {
    std::fs::read(path).ok().map(|data| blake3::hash(&data).to_hex().to_string())
}

pub fn run(source: &str, target: &str, dry_run: bool) -> SyncResult {
    let mut actions = Vec::new();
    let mut copied = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut bytes_synced = 0u64;
    let mut source_count = 0;

    let source_path = std::path::Path::new(source);
    let target_path = std::path::Path::new(target);

    // Ensure target exists
    if !dry_run {
        let _ = std::fs::create_dir_all(target);
    }

    // Walk source and compare with target
    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        source_count += 1;

        let rel = entry.path().strip_prefix(source_path).unwrap_or(entry.path());
        let target_file = target_path.join(rel);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        let src_hash = hash_file(entry.path()).unwrap_or_default();
        let tgt_hash = if target_file.exists() { hash_file(&target_file) } else { None };

        if let Some(ref th) = tgt_hash {
            if *th == src_hash {
                skipped += 1;
                continue;
            }
            // Different hash — update
            actions.push(SyncAction {
                path: rel.to_string_lossy().to_string(),
                action: "update".into(),
                direction: "source → target".into(),
                size_bytes: size,
                source_hash: src_hash.clone(),
                target_hash: tgt_hash.clone(),
            });
            if !dry_run {
                if let Some(parent) = target_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(entry.path(), &target_file);
            }
            updated += 1;
            bytes_synced += size;
        } else {
            // New file — copy
            actions.push(SyncAction {
                path: rel.to_string_lossy().to_string(),
                action: "copy".into(),
                direction: "source → target".into(),
                size_bytes: size,
                source_hash: src_hash.clone(),
                target_hash: None,
            });
            if !dry_run {
                if let Some(parent) = target_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(entry.path(), &target_file);
            }
            copied += 1;
            bytes_synced += size;
        }
    }

    let target_count = WalkDir::new(target).into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();

    SyncResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source: source.to_string(),
        target: target.to_string(),
        dry_run,
        total_source_files: source_count,
        total_target_files: target_count,
        actions,
        copied,
        updated,
        skipped,
        total_bytes_synced: bytes_synced,
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 { format!("{:.2} GB", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.2} MB", bytes as f64 / 1_048_576.0) }
    else if bytes >= 1024 { format!("{:.2} KB", bytes as f64 / 1024.0) }
    else { format!("{} B", bytes) }
}

fn format_pretty(result: &SyncResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  🔗 TELEPATHY-SYNC \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / Directory Sync\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ Sync Summary\n"));
    out.push_str(&format!("    Source: {}\n", result.source));
    out.push_str(&format!("    Target: {}\n", result.target));
    out.push_str(&format!("    Mode: {}\n", if result.dry_run { "DRY RUN" } else { "LIVE" }));
    out.push_str(&format!("    Source Files: {}\n", result.total_source_files));
    out.push_str(&format!("    Target Files: {}\n", result.total_target_files));

    out.push_str("\n");
    out.push_str(&format!("  ▸ Actions\n"));
    out.push_str(&format!("    Copied (new): {}\n", result.copied));
    out.push_str(&format!("    Updated (changed): {}\n", result.updated));
    out.push_str(&format!("    Skipped (identical): {}\n", result.skipped));
    out.push_str(&format!("    Bytes Synced: {}\n", format_size(result.total_bytes_synced)));

    if !result.actions.is_empty() {
        out.push_str("\n");
        out.push_str(&format!("  ▸ Details\n"));
        for a in result.actions.iter().take(20) {
            out.push_str(&format!("    {}: {} ({})\n", a.action, a.path, format_size(a.size_bytes)));
        }
        if result.actions.len() > 20 {
            out.push_str(&format!("    ...: and {} more\n", result.actions.len() - 20));
        }
    }

    out.push_str(&format!("\n  ⟫ telepathy-sync :: {} copied, {} updated, {} skipped\n\n",
        result.copied, result.updated, result.skipped));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.source, &cli.target, cli.dry_run);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("telepathy-sync", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
