use crate::bio_client;
use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "viral-infect", about = "Code transformation propagator — mass regex find-and-replace")]
pub struct Cli {
    /// Source directory to infect
    pub source: String,

    /// Rules JSON file: [{"pattern": "regex", "replacement": "string"}]
    #[arg(long)]
    pub rules: Option<String>,

    /// Regex pattern to match
    #[arg(long)]
    pub pattern: Option<String>,

    /// Replacement string
    #[arg(long)]
    pub replace: Option<String>,

    /// File extension filter (e.g., rs, py, json)
    #[arg(long)]
    pub ext: Option<String>,

    /// Dry run mode (no actual file writes)
    #[arg(long)]
    pub dry_run: bool,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: String,
    pub replacement: String,
}

#[derive(Debug, Serialize)]
pub struct Infection {
    pub file: String,
    pub matches_count: usize,
    pub mutated: bool,
}

#[derive(Debug, Serialize)]
pub struct InfectResult {
    pub timestamp: String,
    pub source: String,
    pub dry_run: bool,
    pub rules_count: usize,
    pub files_scanned: usize,
    pub files_infected: usize,
    pub total_mutations: usize,
    pub virulence_score: f64,
    pub infections: Vec<Infection>,
}

pub fn run(source: &str, rules: &[Rule], ext_filter: Option<&str>, dry_run: bool) -> InfectResult {
    let compiled: Vec<(Regex, &str)> = rules.iter()
        .filter_map(|r| Regex::new(&r.pattern).ok().map(|rx| (rx, r.replacement.as_str())))
        .collect();

    let mut infections = Vec::new();
    let mut files_scanned = 0;
    let mut files_infected = 0;
    let mut total_mutations = 0;

    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }

        if let Some(ext) = ext_filter {
            if entry.path().extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        files_scanned += 1;

        let mut new_content = content.clone();
        let mut match_count = 0;

        for (rx, repl) in &compiled {
            let matches: Vec<_> = rx.find_iter(&new_content).collect();
            match_count += matches.len();
            if !matches.is_empty() {
                new_content = rx.replace_all(&new_content, *repl).to_string();
            }
        }

        if match_count > 0 {
            files_infected += 1;
            total_mutations += match_count;

            if !dry_run && new_content != content {
                let _ = std::fs::write(entry.path(), &new_content);
            }

            infections.push(Infection {
                file: entry.path().to_string_lossy().to_string(),
                matches_count: match_count,
                mutated: !dry_run && new_content != content,
            });
        }
    }

    let virulence = if files_scanned > 0 {
        files_infected as f64 / files_scanned as f64
    } else {
        0.0
    };

    InfectResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source: source.to_string(),
        dry_run,
        rules_count: rules.len(),
        files_scanned,
        files_infected,
        total_mutations,
        virulence_score: (virulence * 1000.0).round() / 1000.0,
        infections,
    }
}

pub fn format_pretty(result: &InfectResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "\u{1f9a0}", "VIRAL-INFECT"));
    out.push_str(&format!("║  Layer: {}\n", "Bio-Evolution / Code Transformer"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  > Infection Summary\n"));
    out.push_str(&format!("    Source: {}\n", result.source));
    out.push_str(&format!("    Mode: {}\n", if result.dry_run { "DRY RUN" } else { "LIVE" }));
    out.push_str(&format!("    Rules: {}\n", result.rules_count));
    out.push_str(&format!("    Files Scanned: {}\n", result.files_scanned));
    out.push_str(&format!("    Files Infected: {}\n", result.files_infected));
    out.push_str(&format!("    Total Mutations: {}\n", result.total_mutations));
    out.push_str(&format!("    Virulence: {:.1}%\n", result.virulence_score * 100.0));

    if !result.infections.is_empty() {
        out.push('\n');
        out.push_str(&format!("  > Infected Files\n"));
        for inf in result.infections.iter().take(20) {
            let status = if inf.mutated { "MUTATED" } else { "DETECTED" };
            out.push_str(&format!("    {}: {} matches [{}]\n", inf.file, inf.matches_count, status));
        }
    }

    out.push_str(&format!("\n  >> viral-infect :: virulence={:.1}%\n\n", result.virulence_score * 100.0));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let rules: Vec<Rule> = if let Some(rules_path) = &cli.rules {
        match std::fs::read_to_string(rules_path) {
            Ok(json) => serde_json::from_str(&json).map_err(|e| e.to_string())?,
            Err(e) => return Err(format!("Cannot read rules file: {}", e)),
        }
    } else if let (Some(pat), Some(repl)) = (&cli.pattern, &cli.replace) {
        vec![Rule { pattern: pat.clone(), replacement: repl.clone() }]
    } else {
        return Err("Provide --rules <file> or --pattern + --replace".to_string());
    };

    let result = run(&cli.source, &rules, cli.ext.as_deref(), cli.dry_run);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("viral-infect", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
