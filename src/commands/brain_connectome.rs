use crate::{bio_client, output};
use clap::Parser;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "brain-connectome", about = "Dependency graph builder — static import analysis")]
pub struct Cli {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: String,

    /// Language: rust, python, js
    #[arg(long, default_value = "rust")]
    pub lang: String,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConnectomeNode {
    pub file: String,
    pub imports: Vec<String>,
    pub imported_by: Vec<String>,
    pub weight: usize,
}

#[derive(Debug, Serialize)]
pub struct ConnectomeEdge {
    pub from: String,
    pub to: String,
    pub import_statement: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectomeResult {
    pub timestamp: String,
    pub root: String,
    pub language: String,
    pub nodes: Vec<ConnectomeNode>,
    pub edges: Vec<ConnectomeEdge>,
    pub total_files: usize,
    pub total_imports: usize,
    pub hub_files: Vec<String>,
}

pub struct LangPattern {
    pub extensions: Vec<&'static str>,
    pub import_regex: Regex,
}

pub fn lang_pattern(lang: &str) -> LangPattern {
    match lang {
        "python" | "py" => LangPattern {
            extensions: vec!["py"],
            import_regex: Regex::new(r"(?m)^(?:from\s+(\S+)\s+import|import\s+(\S+))").unwrap(),
        },
        "js" | "javascript" | "ts" | "typescript" => LangPattern {
            extensions: vec!["js", "ts", "jsx", "tsx", "mjs"],
            import_regex: Regex::new(r#"(?m)(?:import\s+.*?\s+from\s+['"]([^'"]+)['"]|require\s*\(\s*['"]([^'"]+)['"]\s*\))"#).unwrap(),
        },
        _ => LangPattern { // rust default
            extensions: vec!["rs"],
            import_regex: Regex::new(r"(?m)^(?:use\s+(\S+?)(?:::\{|;)|mod\s+(\w+)\s*;)").unwrap(),
        },
    }
}

pub fn run(path: &str, lang: &str) -> ConnectomeResult {
    let pattern = lang_pattern(lang);
    let mut file_imports: HashMap<String, Vec<(String, String)>> = HashMap::new(); // file -> [(import_target, raw_statement)]
    let mut all_files: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        let ext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or("");
        if !pattern.extensions.contains(&ext) { continue; }

        let file_path = entry.path().to_string_lossy().to_string();
        all_files.insert(file_path.clone());

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut imports = Vec::new();
        for cap in pattern.import_regex.captures_iter(&content) {
            let target = cap.get(1).or_else(|| cap.get(2))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if !target.is_empty() {
                imports.push((target, cap.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()));
            }
        }

        file_imports.insert(file_path, imports);
    }

    // Build edges
    let mut edges = Vec::new();
    let mut import_count = HashMap::new(); // target -> count of files importing it

    for (file, imports) in &file_imports {
        for (target, raw) in imports {
            edges.push(ConnectomeEdge {
                from: file.clone(),
                to: target.clone(),
                import_statement: raw.clone(),
            });
            *import_count.entry(target.clone()).or_insert(0usize) += 1;
        }
    }

    // Build nodes
    let mut imported_by_map: HashMap<String, Vec<String>> = HashMap::new();
    for (file, imports) in &file_imports {
        for (target, _) in imports {
            imported_by_map.entry(target.clone()).or_default().push(file.clone());
        }
    }

    let mut nodes: Vec<ConnectomeNode> = all_files.iter().map(|f| {
        let imports: Vec<String> = file_imports.get(f)
            .map(|v| v.iter().map(|(t, _)| t.clone()).collect())
            .unwrap_or_default();
        let imported_by: Vec<String> = imported_by_map.get(f).cloned().unwrap_or_default();
        let weight = imports.len() + imported_by.len();
        ConnectomeNode {
            file: f.clone(),
            imports,
            imported_by,
            weight,
        }
    }).collect();

    nodes.sort_by(|a, b| b.weight.cmp(&a.weight));

    let hub_files: Vec<String> = nodes.iter()
        .filter(|n| n.weight > 3)
        .take(10)
        .map(|n| n.file.clone())
        .collect();

    ConnectomeResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: path.to_string(),
        language: lang.to_string(),
        total_files: all_files.len(),
        total_imports: edges.len(),
        nodes,
        edges,
        hub_files,
    }
}

fn print_pretty(result: &ConnectomeResult) {
    output::banner("BRAIN-CONNECTOME", "Machine-Brain / Dependency Graph", "🧠");

    output::section("Project Summary");
    output::kv("Root", &result.root);
    output::kv("Language", &result.language);
    output::kv("Files", &result.total_files.to_string());
    output::kv("Import Edges", &result.total_imports.to_string());

    if !result.hub_files.is_empty() {
        println!();
        output::section("Hub Files (most connected)");
        for f in &result.hub_files {
            let node = result.nodes.iter().find(|n| n.file == *f).unwrap();
            output::kv(f, &format!("weight={} imports={} imported_by={}", node.weight, node.imports.len(), node.imported_by.len()));
        }
    }

    println!();
    output::section("Top Nodes");
    for n in result.nodes.iter().take(15) {
        output::kv(&n.file, &format!("→{} ←{}", n.imports.len(), n.imported_by.len()));
    }

    output::summary("brain-connectome", &format!("{} files, {} edges", result.total_files, result.total_imports));
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.path, &cli.lang);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("brain-connectome", addr).await {
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
