use crate::bio_client;
use clap::Parser;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "plasmid-inject", about = "File patcher — surgical line-level code injection")]
pub struct Cli {
    /// Target file to patch
    pub target: String,

    /// Start line (1-indexed)
    #[arg(long)]
    pub start: usize,

    /// End line (1-indexed, inclusive)
    #[arg(long)]
    pub end: usize,

    /// Inline fix content (newlines as \n)
    #[arg(long)]
    pub fix: Option<String>,

    /// Fix content from file
    #[arg(long)]
    pub fix_file: Option<String>,

    /// Dry run mode (no actual file writes)
    #[arg(long)]
    pub dry_run: bool,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InjectResult {
    pub timestamp: String,
    pub target: String,
    pub dry_run: bool,
    pub lines_removed: usize,
    pub lines_injected: usize,
    pub hash_before: String,
    pub hash_after: String,
    pub patch_range: String,
    pub success: bool,
    pub error: Option<String>,
}

pub fn run(target: &str, start: usize, end: usize, fix_content: &str, dry_run: bool) -> InjectResult {
    let content = match std::fs::read(target) {
        Ok(c) => c,
        Err(e) => {
            return InjectResult {
                timestamp: chrono::Utc::now().to_rfc3339(),
                target: target.to_string(),
                dry_run,
                lines_removed: 0,
                lines_injected: 0,
                hash_before: String::new(),
                hash_after: String::new(),
                patch_range: format!("{}-{}", start, end),
                success: false,
                error: Some(format!("Cannot read file: {}", e)),
            };
        }
    };

    let hash_before = blake3::hash(&content).to_hex().to_string();
    let text = String::from_utf8_lossy(&content);
    let lines: Vec<&str> = text.lines().collect();

    // Validate range
    if start < 1 || start > lines.len() || end < start || end > lines.len() {
        return InjectResult {
            timestamp: chrono::Utc::now().to_rfc3339(),
            target: target.to_string(),
            dry_run,
            lines_removed: 0,
            lines_injected: 0,
            hash_before: hash_before.clone(),
            hash_after: hash_before,
            patch_range: format!("{}-{}", start, end),
            success: false,
            error: Some(format!("Invalid range {}-{} for file with {} lines", start, end, lines.len())),
        };
    }

    let lines_removed = end - start + 1;
    let fix_lines: Vec<&str> = fix_content.lines().collect();
    let lines_injected = fix_lines.len();

    // Apply patch: remove [start-1..end], insert fix
    let mut new_lines: Vec<&str> = Vec::new();
    new_lines.extend_from_slice(&lines[..start - 1]);
    new_lines.extend_from_slice(&fix_lines);
    if end < lines.len() {
        new_lines.extend_from_slice(&lines[end..]);
    }

    let new_content = new_lines.join("\n") + "\n";
    let hash_after = blake3::hash(new_content.as_bytes()).to_hex().to_string();

    if !dry_run {
        if let Err(e) = std::fs::write(target, &new_content) {
            return InjectResult {
                timestamp: chrono::Utc::now().to_rfc3339(),
                target: target.to_string(),
                dry_run,
                lines_removed,
                lines_injected,
                hash_before,
                hash_after,
                patch_range: format!("{}-{}", start, end),
                success: false,
                error: Some(format!("Cannot write file: {}", e)),
            };
        }
    }

    InjectResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        target: target.to_string(),
        dry_run,
        lines_removed,
        lines_injected,
        hash_before,
        hash_after,
        patch_range: format!("{}-{}", start, end),
        success: true,
        error: None,
    }
}

pub fn format_pretty(result: &InjectResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "\u{1f489}", "PLASMID-INJECT"));
    out.push_str(&format!("║  Layer: {}\n", "Bio-Evolution / File Patcher"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  > Patch Operation\n"));
    out.push_str(&format!("    Target: {}\n", result.target));
    out.push_str(&format!("    Mode: {}\n", if result.dry_run { "DRY RUN" } else { "LIVE" }));
    out.push_str(&format!("    Range: {}\n", result.patch_range));
    out.push_str(&format!("    Lines Removed: {}\n", result.lines_removed));
    out.push_str(&format!("    Lines Injected: {}\n", result.lines_injected));

    out.push('\n');
    out.push_str(&format!("  > Integrity\n"));
    let hash_before_display = if result.hash_before.len() >= 32 { &result.hash_before[..32] } else { &result.hash_before };
    let hash_after_display = if result.hash_after.len() >= 32 { &result.hash_after[..32] } else { &result.hash_after };
    out.push_str(&format!("    Hash Before: {}\n", hash_before_display));
    out.push_str(&format!("    Hash After: {}\n", hash_after_display));

    if let Some(ref err) = result.error {
        out.push_str(&format!("  [ERR] {}\n", err));
    }

    let status = if result.success { "INJECTED" } else { "FAILED" };
    out.push_str(&format!("\n  >> plasmid-inject :: {}\n\n", status));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let fix_content = if let Some(ref fix) = cli.fix {
        fix.replace("\\n", "\n")
    } else if let Some(ref fix_file) = cli.fix_file {
        match std::fs::read_to_string(fix_file) {
            Ok(c) => c,
            Err(e) => return Err(format!("Cannot read fix file: {}", e)),
        }
    } else {
        return Err("Provide --fix <content> or --fix-file <path>".to_string());
    };

    let result = run(&cli.target, cli.start, cli.end, &fix_content, cli.dry_run);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("plasmid-inject", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
