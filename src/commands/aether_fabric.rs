use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use sysinfo::{RefreshKind, System};

#[derive(Parser)]
#[command(name = "aether-fabric", about = "System topology mapper — process/port/connection graph")]
pub struct Cli {
    /// Show top N processes by CPU
    #[arg(long, default_value = "30")]
    pub top: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProcessNode {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
    pub parent_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkConnection {
    pub local_addr: String,
    pub remote_addr: String,
    pub protocol: String,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct FabricResult {
    pub timestamp: String,
    pub hostname: String,
    pub process_count: usize,
    pub top_processes: Vec<ProcessNode>,
    pub network_connections: Vec<NetworkConnection>,
    pub topology_summary: TopologySummary,
}

#[derive(Debug, Serialize)]
pub struct TopologySummary {
    pub total_processes: usize,
    pub total_connections: usize,
    pub listening_ports: Vec<String>,
}

pub fn run(top_n: usize) -> FabricResult {
    let mut sys = System::new_with_specifics(
        RefreshKind::everything(),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    sys.refresh_processes();
    sys.refresh_cpu();

    let mut procs: Vec<ProcessNode> = sys.processes().iter().map(|(pid, p)| {
        ProcessNode {
            pid: pid.as_u32(),
            name: p.name().to_string(),
            cpu_usage: p.cpu_usage(),
            memory_mb: p.memory() / (1024 * 1024),
            parent_pid: p.parent().map(|pp| pp.as_u32()),
        }
    }).collect();

    procs.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
    let total_processes = procs.len();
    procs.truncate(top_n);

    // Get network connections via netstat
    let (connections, listening) = get_network_info();

    FabricResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        process_count: total_processes,
        top_processes: procs,
        network_connections: connections.clone(),
        topology_summary: TopologySummary {
            total_processes,
            total_connections: connections.len(),
            listening_ports: listening,
        },
    }
}

fn get_network_info() -> (Vec<NetworkConnection>, Vec<String>) {
    let mut connections = Vec::new();
    let mut listening = Vec::new();

    // Try netstat on Windows
    if let Ok(out) = std::process::Command::new("netstat").args(["-ano"]).output() {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines().skip(4) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let proto = parts[0].to_string();
                    let local = parts[1].to_string();
                    let remote = parts[2].to_string();
                    let state = if parts.len() > 3 && !parts[3].chars().all(|c| c.is_ascii_digit()) {
                        parts[3].to_string()
                    } else {
                        "".to_string()
                    };

                    if state == "LISTENING" {
                        listening.push(local.clone());
                    }

                    connections.push(NetworkConnection {
                        local_addr: local,
                        remote_addr: remote,
                        protocol: proto,
                        state,
                    });
                }
            }
        }
    }

    connections.truncate(50);
    listening.truncate(30);
    (connections, listening)
}

fn format_pretty(result: &FabricResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  🌐 AETHER-FABRIC \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / Topology Mapper\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ System\n"));
    out.push_str(&format!("    Hostname: {}\n", result.hostname));
    out.push_str(&format!("    Total Processes: {}\n", result.process_count));
    out.push_str(&format!("    Network Connections: {}\n", result.topology_summary.total_connections));

    out.push_str("\n");
    out.push_str(&format!("  ▸ Top Processes (by CPU)\n"));
    for p in result.top_processes.iter().take(15) {
        out.push_str(&format!("    [{}] {}: cpu={:.1}%  mem={}MB\n",
            p.pid, p.name, p.cpu_usage, p.memory_mb));
    }

    if !result.topology_summary.listening_ports.is_empty() {
        out.push_str("\n");
        out.push_str(&format!("  ▸ Listening Ports\n"));
        for port in result.topology_summary.listening_ports.iter().take(15) {
            out.push_str(&format!("    ●: {}\n", port));
        }
    }

    out.push_str(&format!("\n  ⟫ aether-fabric :: {} processes, {} connections\n\n",
        result.process_count, result.topology_summary.total_connections));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(cli.top);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("aether-fabric", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
