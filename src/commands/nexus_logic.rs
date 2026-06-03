use crate::{bio_client, output};
use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, BufRead, Write as IoWrite};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "nexus-logic", about = "Knowledge indexer — local full-text trigram search engine")]
pub struct Cli {
    /// Directory to index
    #[arg(default_value = ".")]
    pub path: String,

    /// File extension filter
    #[arg(long)]
    pub ext: Option<String>,

    /// Search query (trigram-based)
    #[arg(long)]
    pub query: Option<String>,

    /// Limit number of results
    #[arg(long, default_value = "50")]
    pub limit: usize,

    /// Interactive mode
    #[arg(long)]
    pub interactive: bool,

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub file: String,
    pub line: usize,
    pub score: f64,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct IndexStats {
    pub timestamp: String,
    pub root: String,
    pub files_indexed: usize,
    pub total_trigrams: usize,
    pub index_size_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub total_hits: usize,
}

pub struct TrigramIndex {
    // trigram -> vec of (file_id, line_num)
    pub index: HashMap<String, Vec<(usize, usize)>>,
    pub files: Vec<String>,
    pub lines: HashMap<(usize, usize), String>, // (file_id, line_num) -> text
}

pub fn build_trigram(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 3 { return vec![]; }
    (0..chars.len() - 2).map(|i| chars[i..i + 3].iter().collect()).collect()
}

pub fn build_index(path: &str, ext_filter: Option<&str>) -> (TrigramIndex, IndexStats) {
    let mut index: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut files = Vec::new();
    let mut lines_map = HashMap::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        if let Some(ext) = ext_filter {
            if entry.path().extension().and_then(|e| e.to_str()) != Some(ext) { continue; }
        }

        // Only index text-like files
        let fext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(fext, "exe" | "dll" | "bin" | "png" | "jpg" | "gif" | "pdf" | "zip" | "tar" | "gz") {
            continue;
        }

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file_id = files.len();
        files.push(entry.path().to_string_lossy().to_string());

        for (line_num, line) in content.lines().enumerate() {
            let trigrams = build_trigram(line);
            for tri in trigrams {
                index.entry(tri).or_default().push((file_id, line_num));
            }
            lines_map.insert((file_id, line_num), line.to_string());
        }
    }

    let total_trigrams = index.len();
    let index_size: usize = index.iter().map(|(k, v)| k.len() + v.len() * 16).sum();

    let stats = IndexStats {
        timestamp: chrono::Utc::now().to_rfc3339(),
        root: path.to_string(),
        files_indexed: files.len(),
        total_trigrams,
        index_size_bytes: index_size,
    };

    (TrigramIndex { index, files, lines: lines_map }, stats)
}

pub fn search(idx: &TrigramIndex, query: &str, limit: usize) -> SearchResult {
    let query_trigrams = build_trigram(query);
    if query_trigrams.is_empty() {
        return SearchResult { query: query.to_string(), hits: vec![], total_hits: 0 };
    }

    // Score: count how many query trigrams match each (file, line)
    let mut scores: HashMap<(usize, usize), usize> = HashMap::new();
    for tri in &query_trigrams {
        if let Some(locations) = idx.index.get(tri) {
            for &loc in locations {
                *scores.entry(loc).or_insert(0) += 1;
            }
        }
    }

    let mut hits: Vec<((usize, usize), usize)> = scores.into_iter().collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1));

    let total = hits.len();
    let results: Vec<SearchHit> = hits.into_iter().take(limit).map(|((fid, ln), score)| {
        SearchHit {
            file: idx.files.get(fid).cloned().unwrap_or_default(),
            line: ln + 1,
            score: score as f64 / query_trigrams.len() as f64,
            text: idx.lines.get(&(fid, ln)).cloned().unwrap_or_default()
                .chars().take(120).collect(),
        }
    }).collect();

    SearchResult { query: query.to_string(), hits: results, total_hits: total }
}

fn print_pretty_stats(stats: &IndexStats) {
    output::banner("NEXUS-LOGIC", "Machine-Brain / Full-Text Search", "🔍");
    output::section("Index Built");
    output::kv("Root", &stats.root);
    output::kv("Files", &stats.files_indexed.to_string());
    output::kv("Trigrams", &stats.total_trigrams.to_string());
    output::kv("Index Size", &format!("{} bytes", stats.index_size_bytes));
}

fn print_search_result(result: &SearchResult) {
    output::section(&format!("Query: \"{}\" ({} hits)", result.query, result.total_hits));
    for hit in &result.hits {
        output::kv(
            &format!("{}:{}", hit.file, hit.line),
            &format!("[{:.2}] {}", hit.score, hit.text),
        );
    }
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let (idx, stats) = build_index(&cli.path, cli.ext.as_deref());

    print_pretty_stats(&stats);

    if let Some(ref q) = cli.query {
        let result = search(&idx, q, cli.limit);
        print_search_result(&result);
    }

    if cli.interactive {
        println!();
        println!("  Enter queries (Ctrl+C to exit):");
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        loop {
            print!("  > ");
            stdout.flush().unwrap();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).unwrap() == 0 { break; }
            let q = line.trim();
            if q.is_empty() { continue; }
            let result = search(&idx, q, cli.limit);
            print_search_result(&result);
        }
    }

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("nexus-logic", addr).await {
            let result_str = serde_json::to_string(&stats).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    if !cli.interactive && cli.query.is_none() {
        output::summary("nexus-logic", &format!("{} files indexed", stats.files_indexed));
    }

    Ok(String::new())
}
