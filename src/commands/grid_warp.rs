use crate::{bio_client, output};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "grid-warp", about = "Quick-path creator — symlink/junction manager + latency measurement")]
pub struct Cli {
    /// Links specification: JSON array of {"source": "...", "target": "..."}
    #[arg(long)]
    pub links: Option<String>,

    /// Links specification from JSON file
    #[arg(long)]
    pub list: Option<String>,

    /// Source path for single link
    #[arg(long)]
    pub source: Option<String>,

    /// Target path for single link
    #[arg(long)]
    pub target: Option<String>,

    /// Dry run mode (no actual link creation)
    #[arg(long)]
    pub dry_run: bool,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkSpec {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct LinkResult {
    pub source: String,
    pub target: String,
    pub link_type: String,
    pub created: bool,
    pub latency_us: u128,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WarpResult {
    pub timestamp: String,
    pub links: Vec<LinkResult>,
    pub total_created: usize,
    pub total_failed: usize,
    pub avg_latency_us: f64,
}

pub fn create_link(source: &str, target: &str, dry_run: bool) -> LinkResult {
    let src = std::path::Path::new(source);
    let start = Instant::now();

    // Determine link type based on source
    let (link_type, result) = if src.is_dir() {
        // Directory junction on Windows
        if dry_run {
            ("junction (dry)", Ok(()))
        } else {
            let r = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J", target, source])
                .output()
                .map(|_| ())
                .map_err(|e| e.to_string());
            ("junction", r)
        }
    } else {
        // Symlink for files
        if dry_run {
            ("symlink (dry)", Ok(()))
        } else {
            #[cfg(windows)]
            let r = std::os::windows::fs::symlink_file(source, target)
                .map_err(|e| e.to_string());
            #[cfg(not(windows))]
            let r = std::os::unix::fs::symlink(source, target)
                .map_err(|e| e.to_string());
            ("symlink", r)
        }
    };

    let latency = start.elapsed().as_micros();

    // Also measure access latency
    let access_start = Instant::now();
    let _ = std::fs::metadata(if dry_run { source } else { target });
    let access_latency = access_start.elapsed().as_micros();

    match result {
        Ok(()) => LinkResult {
            source: source.to_string(),
            target: target.to_string(),
            link_type: link_type.to_string(),
            created: !dry_run,
            latency_us: latency + access_latency,
            error: None,
        },
        Err(e) => LinkResult {
            source: source.to_string(),
            target: target.to_string(),
            link_type: link_type.to_string(),
            created: false,
            latency_us: latency,
            error: Some(e),
        },
    }
}

pub fn run(specs: &[LinkSpec], dry_run: bool) -> WarpResult {
    let mut results = Vec::new();
    let mut created = 0;
    let mut failed = 0;

    for spec in specs {
        let r = create_link(&spec.source, &spec.target, dry_run);
        if r.created || dry_run { created += 1; } else { failed += 1; }
        results.push(r);
    }

    let avg_latency = if !results.is_empty() {
        results.iter().map(|r| r.latency_us as f64).sum::<f64>() / results.len() as f64
    } else { 0.0 };

    WarpResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        links: results,
        total_created: created,
        total_failed: failed,
        avg_latency_us: (avg_latency * 10.0).round() / 10.0,
    }
}

fn print_pretty(result: &WarpResult) {
    output::banner("GRID-WARP", "Resonance / Symlink Manager", "🌀");

    output::section("Links");
    for link in &result.links {
        let status = if link.error.is_some() { "FAIL" } else if link.created { "CREATED" } else { "DRY" };
        output::kv(
            &format!("{} → {}", link.source, link.target),
            &format!("[{}] {} {}μs", status, link.link_type, link.latency_us),
        );
        if let Some(ref err) = link.error {
            output::error(&format!("  {}", err));
        }
    }

    println!();
    output::kv("Created", &result.total_created.to_string());
    output::kv("Failed", &result.total_failed.to_string());
    output::kv("Avg Latency", &format!("{:.1}μs", result.avg_latency_us));

    output::summary("grid-warp", &format!("{} links, {:.1}μs avg", result.total_created, result.avg_latency_us));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let specs: Vec<LinkSpec> = if let Some(ref json) = cli.links {
        serde_json::from_str(json).map_err(|e| format!("Invalid links JSON: {}", e))?
    } else if let Some(ref list_file) = cli.list {
        // Load links from file
        let json_str = std::fs::read_to_string(list_file)
            .map_err(|e| format!("Cannot read links file: {}", e))?;
        serde_json::from_str(&json_str).map_err(|e| format!("Invalid links JSON: {}", e))?
    } else if let (Some(src), Some(tgt)) = (&cli.source, &cli.target) {
        vec![LinkSpec { source: src.clone(), target: tgt.clone() }]
    } else {
        return Err("Provide --source + --target, --links <json>, or --list <file>".to_string());
    };

    let result = run(&specs, cli.dry_run);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("grid-warp", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    print_pretty(&result);
    Ok(String::new())
}
