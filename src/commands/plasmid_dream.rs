use crate::bio_client;
use clap::Parser;
use regex::Regex;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "plasmid-dream", about = "Predictive error analyzer — build runner + trend analysis")]
pub struct Cli {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: String,

    /// Check command to run (e.g., "cargo check", "npm test")
    #[arg(long, default_value = "cargo check")]
    pub command: String,

    /// Number of future segments to predict
    #[arg(long, default_value = "3")]
    pub predict: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DetectedIssue {
    pub level: String, // error, warning, info
    pub file: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PredictedRisk {
    pub segment: usize,
    pub risk_level: String,
    pub estimated_errors: f64,
    pub estimated_warnings: f64,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct DreamResult {
    pub timestamp: String,
    pub project_path: String,
    pub command: String,
    pub exit_code: i32,
    pub issues: Vec<DetectedIssue>,
    pub error_count: usize,
    pub warning_count: usize,
    pub predictions: Vec<PredictedRisk>,
    pub health_trajectory: String,
}

pub fn parse_issues(output: &str) -> Vec<DetectedIssue> {
    let error_re = Regex::new(r"(?i)error(?:\[E\d+\])?:?\s*(.+)").unwrap();
    let warning_re = Regex::new(r"(?i)warning(?:\[W\d+\])?:?\s*(.+)").unwrap();
    let location_re = Regex::new(r"-->\s*(.+?):(\d+)").unwrap();

    let mut issues = Vec::new();
    let lines: Vec<&str> = output.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let (level, msg) = if let Some(cap) = error_re.captures(line) {
            ("error".to_string(), cap[1].to_string())
        } else if let Some(cap) = warning_re.captures(line) {
            ("warning".to_string(), cap[1].to_string())
        } else {
            continue;
        };

        // Try to find file location in nearby lines
        let mut file = None;
        let mut line_num = None;
        for j in i..=(i + 3).min(lines.len() - 1) {
            if let Some(loc) = location_re.captures(lines[j]) {
                file = Some(loc[1].to_string());
                line_num = loc[2].parse().ok();
                break;
            }
        }

        issues.push(DetectedIssue { level, file, line: line_num, message: msg.trim().to_string() });
    }

    issues
}

pub fn predict_trend(errors: usize, warnings: usize, segments: usize) -> Vec<PredictedRisk> {
    let mut predictions = Vec::new();
    let err_f = errors as f64;
    let warn_f = warnings as f64;

    for seg in 1..=segments {
        // Simple exponential trend: each segment compounds issues by 1.3x
        let factor = 1.3f64.powi(seg as i32);
        let est_errors = err_f * factor;
        let est_warnings = warn_f * factor;
        let total_risk = est_errors * 3.0 + est_warnings;

        let risk_level = if total_risk > 50.0 { "CRITICAL" }
            else if total_risk > 20.0 { "HIGH" }
            else if total_risk > 5.0 { "MODERATE" }
            else { "LOW" };

        let desc = if est_errors > err_f * 2.0 {
            format!("Error count may double — refactoring needed")
        } else if est_warnings > 10.0 {
            format!("Warning accumulation risk — address warnings now")
        } else {
            format!("Stable trajectory — maintain current quality")
        };

        predictions.push(PredictedRisk {
            segment: seg,
            risk_level: risk_level.to_string(),
            estimated_errors: (est_errors * 10.0).round() / 10.0,
            estimated_warnings: (est_warnings * 10.0).round() / 10.0,
            description: desc,
        });
    }

    predictions
}

pub fn run(path: &str, command: &str, predict_segments: usize) -> DreamResult {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let (cmd, cmd_args) = parts.split_first().unwrap_or((&"echo", &[]));

    let result = std::process::Command::new(cmd)
        .args(cmd_args)
        .current_dir(path)
        .output();

    let (exit_code, combined_output) = match result {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            (code, format!("{}\n{}", stdout, stderr))
        }
        Err(e) => (-1, format!("Failed to run command: {}", e)),
    };

    let issues = parse_issues(&combined_output);
    let error_count = issues.iter().filter(|i| i.level == "error").count();
    let warning_count = issues.iter().filter(|i| i.level == "warning").count();
    let predictions = predict_trend(error_count, warning_count, predict_segments);

    let trajectory = if error_count == 0 && warning_count == 0 { "HEALTHY" }
        else if error_count == 0 { "STABLE_WITH_WARNINGS" }
        else if error_count <= 3 { "DEGRADING" }
        else { "CRITICAL" };

    DreamResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_path: path.to_string(),
        command: command.to_string(),
        exit_code,
        issues,
        error_count,
        warning_count,
        predictions,
        health_trajectory: trajectory.to_string(),
    }
}

pub fn format_pretty(result: &DreamResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "\u{1f4ad}", "PLASMID-DREAM"));
    out.push_str(&format!("║  Layer: {}\n", "Bio-Evolution / Predictive Analyzer"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  > Build Analysis\n"));
    out.push_str(&format!("    Path: {}\n", result.project_path));
    out.push_str(&format!("    Command: {}\n", result.command));
    out.push_str(&format!("    Exit Code: {}\n", result.exit_code));
    out.push_str(&format!("    Errors: {}\n", result.error_count));
    out.push_str(&format!("    Warnings: {}\n", result.warning_count));

    if !result.issues.is_empty() {
        out.push('\n');
        out.push_str(&format!("  > Detected Issues\n"));
        for issue in result.issues.iter().take(15) {
            let loc = match (&issue.file, &issue.line) {
                (Some(f), Some(l)) => format!("{}:{}", f, l),
                (Some(f), None) => f.clone(),
                _ => "?".to_string(),
            };
            let prefix = if issue.level == "error" { "[ERR]" } else { "[WRN]" };
            out.push_str(&format!("    {}: {} — {}\n", prefix, loc, issue.message));
        }
    }

    out.push('\n');
    out.push_str(&format!("  > Future Predictions\n"));
    for pred in &result.predictions {
        out.push_str(&format!(
            "    Segment +{}: [{}] err≈{:.1} warn≈{:.1} — {}\n",
            pred.segment, pred.risk_level, pred.estimated_errors, pred.estimated_warnings, pred.description,
        ));
    }

    out.push_str(&format!("\n  >> plasmid-dream :: {}\n\n", result.health_trajectory));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let result = run(&cli.path, &cli.command, cli.predict);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("plasmid-dream", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
