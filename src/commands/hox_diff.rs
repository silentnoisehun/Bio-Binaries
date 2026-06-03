use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "hox-diff", about = "Project structure differentiator — anatomical region mapper")]
pub struct Cli {
    /// Project root directory
    #[arg(default_value = ".")]
    pub path: String,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AnatomyRegion {
    pub name: String,
    pub region_type: String, // HEAD, THORAX, TAIL, LIMBS
    pub directories: Vec<String>,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub dominant_extensions: Vec<(String, usize)>,
    pub stability: f64, // 0.0-1.0 based on recent changes
}

#[derive(Debug, Serialize)]
pub struct HoxResult {
    pub timestamp: String,
    pub project_root: String,
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_size_bytes: u64,
    pub regions: Vec<AnatomyRegion>,
    pub stability_score: f64,
    pub anatomy_class: String,
}

pub fn classify_region(dir_name: &str) -> &'static str {
    let lower = dir_name.to_lowercase();
    // HEAD: UI, frontend, views, templates
    if matches!(lower.as_str(), "ui" | "frontend" | "views" | "templates" | "pages" | "components"
        | "public" | "static" | "assets" | "styles" | "css") {
        return "HEAD";
    }
    // THORAX: backend, core, lib, engine
    if matches!(lower.as_str(), "src" | "lib" | "core" | "engine" | "backend" | "server"
        | "api" | "services" | "handlers" | "controllers" | "models" | "domain") {
        return "THORAX";
    }
    // TAIL: config, settings, build
    if matches!(lower.as_str(), "config" | "settings" | "build" | "scripts" | "ci" | ".github"
        | ".vscode" | "docker" | "deploy" | "infra" | "terraform") {
        return "TAIL";
    }
    // LIMBS: I/O, data, tests, docs
    if matches!(lower.as_str(), "tests" | "test" | "spec" | "docs" | "doc" | "data"
        | "fixtures" | "migrations" | "seeds" | "input" | "output" | "logs") {
        return "LIMBS";
    }
    "THORAX" // default
}

pub fn run(path: &str) -> HoxResult {
    let now = std::time::SystemTime::now();
    let mut region_data: HashMap<String, (Vec<String>, usize, u64, HashMap<String, usize>, Vec<f64>)> = HashMap::new();
    let mut total_files = 0;
    let mut total_dirs = 0;
    let mut total_size = 0u64;

    for entry in WalkDir::new(path).max_depth(8).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
        let top_dir = rel.components().next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        if entry.file_type().is_dir() {
            total_dirs += 1;
            continue;
        }

        total_files += 1;
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        total_size += size;

        let region_type = classify_region(&top_dir);
        let ext = entry.path().extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "none".to_string());

        let age_days = meta.and_then(|m| m.modified().ok())
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(365.0);

        let entry_data = region_data.entry(region_type.to_string())
            .or_insert_with(|| (Vec::new(), 0, 0, HashMap::new(), Vec::new()));

        if !entry_data.0.contains(&top_dir) {
            entry_data.0.push(top_dir);
        }
        entry_data.1 += 1;
        entry_data.2 += size;
        *entry_data.3.entry(ext).or_insert(0) += 1;
        entry_data.4.push(age_days);
    }

    let mut regions: Vec<AnatomyRegion> = region_data.into_iter().map(|(rtype, (dirs, count, size, exts, ages))| {
        let mut ext_vec: Vec<(String, usize)> = exts.into_iter().collect();
        ext_vec.sort_by(|a, b| b.1.cmp(&a.1));
        ext_vec.truncate(5);

        // Stability: based on average age (older = more stable)
        let avg_age = if !ages.is_empty() { ages.iter().sum::<f64>() / ages.len() as f64 } else { 0.0 };
        let stability = (avg_age / 365.0).min(1.0); // normalize to 1 year

        AnatomyRegion {
            name: rtype.clone(),
            region_type: rtype,
            directories: dirs,
            file_count: count,
            total_size_bytes: size,
            dominant_extensions: ext_vec,
            stability: (stability * 1000.0).round() / 1000.0,
        }
    }).collect();

    regions.sort_by(|a, b| b.file_count.cmp(&a.file_count));

    let avg_stability: f64 = if !regions.is_empty() {
        regions.iter().map(|r| r.stability).sum::<f64>() / regions.len() as f64
    } else { 0.0 };

    let anatomy_class = if regions.len() >= 4 { "COMPLETE_ORGANISM" }
        else if regions.len() >= 3 { "PARTIAL_ORGANISM" }
        else if regions.len() >= 2 { "BILATERAL" }
        else { "UNICELLULAR" };

    HoxResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_root: path.to_string(),
        total_files,
        total_dirs,
        total_size_bytes: total_size,
        regions,
        stability_score: (avg_stability * 1000.0).round() / 1000.0,
        anatomy_class: anatomy_class.to_string(),
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 { format!("{:.2} GB", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.2} MB", bytes as f64 / 1_048_576.0) }
    else if bytes >= 1024 { format!("{:.2} KB", bytes as f64 / 1024.0) }
    else { format!("{} B", bytes) }
}

pub fn format_pretty(result: &HoxResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "\u{1f9b4}", "HOX-DIFF"));
    out.push_str(&format!("║  Layer: {}\n", "Bio-Evolution / Project Anatomy"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  > Project Overview\n"));
    out.push_str(&format!("    Root: {}\n", result.project_root));
    out.push_str(&format!("    Files: {}\n", result.total_files));
    out.push_str(&format!("    Directories: {}\n", result.total_dirs));
    out.push_str(&format!("    Total Size: {}\n", format_size(result.total_size_bytes)));
    out.push_str(&format!("    Anatomy Class: {}\n", result.anatomy_class));
    out.push_str(&format!("    Stability: {:.3}\n", result.stability_score));

    for region in &result.regions {
        out.push('\n');
        out.push_str(&format!("  > {} ({})\n", region.name, region.region_type));
        out.push_str(&format!("    Directories: {}\n", region.directories.join(", ")));
        out.push_str(&format!("    Files: {}\n", region.file_count));
        out.push_str(&format!("    Size: {}\n", format_size(region.total_size_bytes)));
        out.push_str(&format!("    Stability: {:.3}\n", region.stability));
        let exts: Vec<String> = region.dominant_extensions.iter()
            .map(|(e, c)| format!(".{}({})", e, c)).collect();
        out.push_str(&format!("    Types: {}\n", exts.join(" ")));
    }

    out.push_str(&format!("\n  >> hox-diff :: {}\n\n", result.anatomy_class));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let result = run(&cli.path);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("hox-diff", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
