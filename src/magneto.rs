/// Magneto — shared code-quality scanning logic
///
/// Extracts error/warning hotspots from source files using regex patterns.
/// Each pattern has a magnetic charge (negative = repulsive/error).
/// Used by magneto-geo (visual) and magneto-acoustic (audio) binaries.

use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Clone)]
pub struct Hotspot {
    pub file: String,
    pub line: usize,
    pub pattern: String,
    pub severity: String,
    pub text: String,
    pub magnetic_charge: f64,
}

#[derive(Debug, Serialize)]
pub struct GeoResult {
    pub timestamp: String,
    pub root: String,
    pub files_scanned: usize,
    pub total_hotspots: usize,
    pub hotspots: Vec<Hotspot>,
    pub severity_counts: std::collections::HashMap<String, usize>,
    pub tension_score: f64,
}

pub struct PatternDef {
    pub regex: Regex,
    pub name: &'static str,
    pub severity: &'static str,
    pub charge: f64,
}

pub fn build_patterns() -> Vec<PatternDef> {
    vec![
        PatternDef { regex: Regex::new(r"(?i)\bERROR\b").unwrap(), name: "ERROR", severity: "high", charge: -1.0 },
        PatternDef { regex: Regex::new(r"(?i)\bWARNING\b").unwrap(), name: "WARNING", severity: "medium", charge: -0.5 },
        PatternDef { regex: Regex::new(r"\bDONE\b").unwrap(), name: "DONE", severity: "low", charge: -0.3 },
        PatternDef { regex: Regex::new(r"\bFIXME\b").unwrap(), name: "FIXME", severity: "medium", charge: -0.7 },
        PatternDef { regex: Regex::new(r"\bHACK\b").unwrap(), name: "HACK", severity: "medium", charge: -0.8 },
        PatternDef { regex: Regex::new(r"\bBUG\b").unwrap(), name: "BUG", severity: "high", charge: -1.0 },
        PatternDef { regex: Regex::new(r"(?i)\bpanic\b").unwrap(), name: "PANIC", severity: "critical", charge: -1.5 },
        PatternDef { regex: Regex::new(r"(?i)\bunwrap\(\)").unwrap(), name: "UNWRAP", severity: "low", charge: -0.2 },
        PatternDef { regex: Regex::new(r"(?i)\bunsafe\b").unwrap(), name: "UNSAFE", severity: "medium", charge: -0.6 },
    ]
}

pub fn is_text_file(path: &std::path::Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(ext,
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "c" | "cpp" | "h" |
            "go" | "rb" | "php" | "cs" | "swift" | "kt" | "scala" | "toml" | "yaml" |
            "yml" | "json" | "xml" | "html" | "css" | "md" | "txt" | "sh" | "bat" | "ps1"
        ),
        None => false,
    }
}

pub fn run(path: &str, depth: usize) -> GeoResult {
    let patterns = build_patterns();
    let mut hotspots = Vec::new();
    let mut severity_counts = std::collections::HashMap::new();
    let mut files_scanned = 0;

    for entry in WalkDir::new(path).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        if !is_text_file(entry.path()) { continue; }

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        files_scanned += 1;

        for (line_num, line) in content.lines().enumerate() {
            for pat in &patterns {
                if pat.regex.is_match(line) {
                    *severity_counts.entry(pat.severity.to_string()).or_insert(0) += 1;
                    hotspots.push(Hotspot {
                        file: entry.path().to_string_lossy().to_string(),
                        line: line_num + 1,
                        pattern: pat.name.to_string(),
                        severity: pat.severity.to_string(),
                        text: line.trim().chars().take(120).collect(),
                        magnetic_charge: pat.charge,
                    });
                }
            }
        }
    }

    hotspots.sort_by(|a, b| a.magnetic_charge.partial_cmp(&b.magnetic_charge).unwrap_or(std::cmp::Ordering::Equal));

    let tension: f64 = hotspots.iter().map(|h| h.magnetic_charge.abs()).sum();
    let tension_score = if files_scanned > 0 {
        tension / files_scanned as f64
    } else {
        0.0
    };

    GeoResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: path.to_string(),
        files_scanned,
        total_hotspots: hotspots.len(),
        hotspots,
        severity_counts,
        tension_score: (tension_score * 100.0).round() / 100.0,
    }
}
