use crate::{bio_client, output};
use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "brain-synapse", about = "File co-change tracker — Hebbian git analysis")]
pub struct Cli {
    /// Git repository directory
    #[arg(default_value = ".")]
    pub path: String,

    /// Number of git log entries to analyze
    #[arg(long, default_value = "500")]
    pub limit: usize,

    /// Minimum co-change count to include
    #[arg(long, default_value = "2")]
    pub min_weight: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SynapticLink {
    pub file_a: String,
    pub file_b: String,
    pub co_change_count: usize,
    pub hebbian_weight: f64,
    pub last_co_change: String,
}

#[derive(Debug, Serialize)]
pub struct SynapseResult {
    pub timestamp: String,
    pub repo_path: String,
    pub commits_analyzed: usize,
    pub unique_files: usize,
    pub synaptic_links: Vec<SynapticLink>,
    pub strongest_clusters: Vec<Vec<String>>,
}

pub fn run(path: &str, limit: usize, min_weight: usize) -> SynapseResult {
    // Get git log with file lists
    let output_raw = std::process::Command::new("git")
        .args([
            "log",
            "--pretty=format:%H|%aI",
            "--name-only",
            &format!("-{}", limit),
        ])
        .current_dir(path)
        .output();

    let git_output = match output_raw {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => {
            return SynapseResult {
                timestamp: chrono::Utc::now().to_rfc3339(),
                repo_path: path.to_string(),
                commits_analyzed: 0,
                unique_files: 0,
                synaptic_links: Vec::new(),
                strongest_clusters: Vec::new(),
            };
        }
    };

    // Parse commits and their file lists
    let mut commits: Vec<(String, Vec<String>)> = Vec::new(); // (date, files)
    let mut current_date = String::new();
    let mut current_files = Vec::new();

    for line in git_output.lines() {
        if line.contains('|') {
            // New commit header
            if !current_files.is_empty() {
                commits.push((current_date.clone(), current_files.clone()));
                current_files.clear();
            }
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            current_date = parts.get(1).unwrap_or(&"").to_string();
        } else if !line.trim().is_empty() {
            current_files.push(line.trim().to_string());
        }
    }
    if !current_files.is_empty() {
        commits.push((current_date, current_files));
    }

    // Build co-change matrix (Hebbian: files that change together, wire together)
    let mut co_changes: HashMap<(String, String), (usize, String)> = HashMap::new();
    let mut all_files = std::collections::HashSet::new();

    for (date, files) in &commits {
        for f in files {
            all_files.insert(f.clone());
        }
        // All pairs in this commit
        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                let (a, b) = if files[i] < files[j] {
                    (files[i].clone(), files[j].clone())
                } else {
                    (files[j].clone(), files[i].clone())
                };
                let entry = co_changes.entry((a, b)).or_insert((0, String::new()));
                entry.0 += 1;
                if entry.1.is_empty() || date > &entry.1 {
                    entry.1 = date.clone();
                }
            }
        }
    }

    // Build synaptic links with Hebbian weights
    let max_co = co_changes.values().map(|(c, _)| *c).max().unwrap_or(1) as f64;
    let mut links: Vec<SynapticLink> = co_changes.into_iter()
        .filter(|(_, (count, _))| *count >= min_weight)
        .map(|((a, b), (count, last))| {
            SynapticLink {
                file_a: a,
                file_b: b,
                co_change_count: count,
                hebbian_weight: (count as f64 / max_co * 1000.0).round() / 1000.0,
                last_co_change: last,
            }
        })
        .collect();

    links.sort_by(|a, b| b.co_change_count.cmp(&a.co_change_count));

    // Find strongest clusters (connected components of top links)
    let top_links: Vec<_> = links.iter().take(20).collect();
    let mut clusters: Vec<Vec<String>> = Vec::new();
    for link in &top_links {
        let mut found = false;
        for cluster in &mut clusters {
            if cluster.contains(&link.file_a) || cluster.contains(&link.file_b) {
                if !cluster.contains(&link.file_a) { cluster.push(link.file_a.clone()); }
                if !cluster.contains(&link.file_b) { cluster.push(link.file_b.clone()); }
                found = true;
                break;
            }
        }
        if !found {
            clusters.push(vec![link.file_a.clone(), link.file_b.clone()]);
        }
    }

    SynapseResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        repo_path: path.to_string(),
        commits_analyzed: commits.len(),
        unique_files: all_files.len(),
        synaptic_links: links,
        strongest_clusters: clusters,
    }
}

fn print_pretty(result: &SynapseResult) {
    output::banner("BRAIN-SYNAPSE", "Machine-Brain / Co-Change Tracker", "⚡");

    output::section("Repository Analysis");
    output::kv("Path", &result.repo_path);
    output::kv("Commits Analyzed", &result.commits_analyzed.to_string());
    output::kv("Unique Files", &result.unique_files.to_string());
    output::kv("Synaptic Links", &result.synaptic_links.len().to_string());

    println!();
    output::section("Strongest Synaptic Links (Top 15)");
    for link in result.synaptic_links.iter().take(15) {
        output::kv(
            &format!("{} ↔ {}", link.file_a, link.file_b),
            &format!("co-changes={} weight={:.3}", link.co_change_count, link.hebbian_weight),
        );
    }

    if !result.strongest_clusters.is_empty() {
        println!();
        output::section("File Clusters (change together)");
        for (i, cluster) in result.strongest_clusters.iter().enumerate() {
            output::kv(&format!("Cluster {}", i + 1), &cluster.join(", "));
        }
    }

    output::summary("brain-synapse", &format!("{} links from {} commits", result.synaptic_links.len(), result.commits_analyzed));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.path, cli.limit, cli.min_weight);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("brain-synapse", addr).await {
            let files_str = result.unique_files.to_string();
            let links_str = result.synaptic_links.len().to_string();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("unique_files", files_str.as_bytes()),
                ("synaptic_links", links_str.as_bytes()),
            ]).await;
        }
    }

    print_pretty(&result);
    Ok(String::new())
}
