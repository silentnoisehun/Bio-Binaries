use crate::{bio_client, output};
use clap::Parser;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "path-resonance", about = "Hot-path detector — filesystem activity heatmap")]
pub struct Cli {
    /// Directory to analyze
    #[arg(default_value = ".")]
    pub path: String,

    /// Maximum depth
    #[arg(long, default_value = "5")]
    pub depth: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PathEntry {
    pub path: String,
    pub last_modified_days: f64,
    pub last_accessed_days: f64,
    pub activity_score: f64,
    pub resonance_type: String, // "constructive" (hot), "destructive" (cold)
    pub recommendation: String,
}

#[derive(Debug, Serialize)]
pub struct ResonanceResult {
    pub timestamp: String,
    pub root: String,
    pub total_paths: usize,
    pub hot_paths: usize,
    pub cold_paths: usize,
    pub paths: Vec<PathEntry>,
}

pub fn run(path: &str, depth: usize) -> ResonanceResult {
    let now = std::time::SystemTime::now();
    let mut paths = Vec::new();
    let mut hot = 0;
    let mut cold = 0;

    for entry in WalkDir::new(path).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let mod_age = meta.modified().ok()
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(9999.0);

        let acc_age = meta.accessed().ok()
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(9999.0);

        // Activity score: lower age = higher activity
        let activity = 100.0 / (1.0 + (mod_age + acc_age) / 2.0);

        let (res_type, recommendation) = if activity > 50.0 {
            hot += 1;
            ("constructive".to_string(), "HOT — actively used".to_string())
        } else if activity > 10.0 {
            ("neutral".to_string(), "WARM — moderate use".to_string())
        } else if activity > 1.0 {
            cold += 1;
            ("destructive".to_string(), "COLD — consider archiving".to_string())
        } else {
            cold += 1;
            ("destructive".to_string(), "FROZEN — candidate for cleanup".to_string())
        };

        paths.push(PathEntry {
            path: entry.path().to_string_lossy().to_string(),
            last_modified_days: (mod_age * 10.0).round() / 10.0,
            last_accessed_days: (acc_age * 10.0).round() / 10.0,
            activity_score: (activity * 10.0).round() / 10.0,
            resonance_type: res_type,
            recommendation,
        });
    }

    paths.sort_by(|a, b| b.activity_score.partial_cmp(&a.activity_score).unwrap_or(std::cmp::Ordering::Equal));

    ResonanceResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: path.to_string(),
        total_paths: paths.len(),
        hot_paths: hot,
        cold_paths: cold,
        paths,
    }
}

fn print_pretty(result: &ResonanceResult) {
    output::banner("PATH-RESONANCE", "Resonance / Hot-Path Detector", "🔥");

    output::section("Heatmap Summary");
    output::kv("Root", &result.root);
    output::kv("Total Paths", &result.total_paths.to_string());
    output::kv("Hot Paths", &result.hot_paths.to_string());
    output::kv("Cold Paths", &result.cold_paths.to_string());

    println!();
    output::section("Hottest Paths (Top 15)");
    for p in result.paths.iter().take(15) {
        output::status(&p.path, p.activity_score, "");
        println!("      {} modified={:.1}d accessed={:.1}d",
            p.recommendation, p.last_modified_days, p.last_accessed_days);
    }

    if result.cold_paths > 0 {
        println!();
        output::section("Coldest Paths (Bottom 10)");
        for p in result.paths.iter().rev().take(10) {
            output::kv(&p.path, &p.recommendation);
        }
    }

    output::summary("path-resonance", &format!("{} hot, {} cold", result.hot_paths, result.cold_paths));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.path, cli.depth);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("path-resonance", addr).await {
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
