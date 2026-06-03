use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "mycelium-spread", about = "Recursive filesystem mapper — builds a network graph of directories")]
pub struct Cli {
    /// Root directory to scan
    #[arg(default_value = ".")]
    pub root: String,

    /// Maximum depth
    #[arg(long, default_value = "6")]
    pub depth: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MyceliumNode {
    pub path: String,
    pub node_type: String, // "directory" or "file"
    pub size_bytes: u64,
    pub children_count: usize,
    pub file_types: HashMap<String, usize>,
    pub age_days: f64,
}

#[derive(Debug, Serialize)]
pub struct SpreadResult {
    pub timestamp: String,
    pub root: String,
    pub total_nodes: usize,
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_size_bytes: u64,
    pub hyphae: Vec<MyceliumNode>,
    pub stabilization_points: Vec<String>,
    pub type_distribution: HashMap<String, usize>,
}

pub fn run(root: &str, max_depth: usize) -> SpreadResult {
    let mut hyphae = Vec::new();
    let mut total_files = 0usize;
    let mut total_dirs = 0usize;
    let mut total_size = 0u64;
    let mut global_types: HashMap<String, usize> = HashMap::new();
    let mut dir_sizes: HashMap<String, u64> = HashMap::new();
    let now = std::time::SystemTime::now();

    for entry in WalkDir::new(root).max_depth(max_depth).into_iter().filter_map(|e| e.ok()) {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let path = entry.path().to_string_lossy().to_string();
        let age = meta.modified().ok()
            .and_then(|m| now.duration_since(m).ok())
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(0.0);

        if meta.is_dir() {
            total_dirs += 1;
            let children = std::fs::read_dir(entry.path())
                .map(|rd| rd.count()).unwrap_or(0);
            let mut file_types = HashMap::new();

            for child in WalkDir::new(entry.path()).max_depth(1).into_iter().filter_map(|e| e.ok()) {
                if child.path() != entry.path() {
                    if let Some(ext) = child.path().extension() {
                        *file_types.entry(ext.to_string_lossy().to_string()).or_insert(0) += 1;
                    }
                }
            }

            hyphae.push(MyceliumNode {
                path: path.clone(),
                node_type: "directory".into(),
                size_bytes: 0,
                children_count: children,
                file_types,
                age_days: (age * 10.0).round() / 10.0,
            });
        } else {
            total_files += 1;
            let size = meta.len();
            total_size += size;

            if let Some(ext) = entry.path().extension() {
                let ext_str = ext.to_string_lossy().to_string();
                *global_types.entry(ext_str).or_insert(0) += 1;
            }

            // Accumulate size for parent dirs
            if let Some(parent) = entry.path().parent() {
                *dir_sizes.entry(parent.to_string_lossy().to_string()).or_insert(0) += size;
            }
        }
    }

    // Find stabilization points: directories with >50% of total size
    let threshold = total_size / 2;
    let mut stabilization_points: Vec<String> = dir_sizes.iter()
        .filter(|(_, &s)| s > threshold / 10) // top dirs by accumulated size
        .map(|(p, _)| p.clone())
        .collect();
    stabilization_points.sort();
    stabilization_points.truncate(10);

    // Update hyphae with accumulated sizes
    for node in &mut hyphae {
        if node.node_type == "directory" {
            if let Some(&size) = dir_sizes.get(&node.path) {
                node.size_bytes = size;
            }
        }
    }

    SpreadResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: root.to_string(),
        total_nodes: total_files + total_dirs,
        total_files,
        total_dirs,
        total_size_bytes: total_size,
        hyphae,
        stabilization_points,
        type_distribution: global_types,
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 { format!("{:.2} GB", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.2} MB", bytes as f64 / 1_048_576.0) }
    else if bytes >= 1024 { format!("{:.2} KB", bytes as f64 / 1024.0) }
    else { format!("{} B", bytes) }
}

fn format_pretty(result: &SpreadResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "🍄", "MYCELIUM-SPREAD"));
    out.push_str(&format!("║  Layer: {}\n", "Resonance / Filesystem Mapper"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str("  ▸ Scan Summary\n");
    out.push_str(&format!("    Root: {}\n", result.root));
    out.push_str(&format!("    Total Nodes: {}\n", result.total_nodes));
    out.push_str(&format!("    Files: {}\n", result.total_files));
    out.push_str(&format!("    Directories: {}\n", result.total_dirs));
    out.push_str(&format!("    Total Size: {}\n", format_size(result.total_size_bytes)));

    out.push('\n');
    out.push_str("  ▸ Top File Types\n");
    let mut types: Vec<_> = result.type_distribution.iter().collect();
    types.sort_by(|a, b| b.1.cmp(a.1));
    for (ext, count) in types.iter().take(10) {
        out.push_str(&format!("    .{}: {}\n", ext, count));
    }

    if !result.stabilization_points.is_empty() {
        out.push('\n');
        out.push_str("  ▸ Stabilization Points (large directories)\n");
        for sp in &result.stabilization_points {
            out.push_str(&format!("    ●: {}\n", sp));
        }
    }

    out.push_str(&format!("\n  ⟫ mycelium-spread :: {} nodes mapped\n\n", result.total_nodes));
    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.root, cli.depth);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("mycelium-spread", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
