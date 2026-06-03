use crate::bio_protocol::{self, BioMessage, BioOp, NonceWindow};
use crate::auth::QueenKey;
use crate::output;
use crate::wave_store::now_ms;
use clap::{Parser, Subcommand};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(name = "omega-master", about = "Echo-X Queen — Central Orchestrator (v2: DNA Protocol)")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// UDP port to listen on
    #[arg(long, default_value = "8888")]
    port: u16,

    /// Directory for persistent state (.bio-queen.key, drone registry, etc.)
    #[arg(long, default_value = ".")]
    state_dir: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Queen server (listen for drones)
    Start {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Query drone status
    Status {
        /// Drone name filter (optional)
        name: Option<String>,
    },
    /// Run a task on all drones
    RunAll {
        /// Task payload
        task: String,
    },
    /// Send APOPTOSIS signal to all drones
    Apoptosis,
    /// Show Queen key info
    KeyInfo,
    /// Freeze all drones
    Freeze,
    /// Thaw all drones
    Thaw,
    /// Query Microscope Memory
    Microscope {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Trigger homeostasis evaluation
    Homeo,
}

// ── Registry ──

/// A single drone in the registry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DroneEntry {
    pub name: String,
    pub addr: String,
    pub generation: u32,
    pub status: String,
    pub protocol: String, // "v2-bio" or "v1-echox" (legacy during migration)
    pub last_heartbeat: u64,
    pub joined_at: u64,
}

/// Shared drone registry (arc-wrapped for async sharing)
pub type DroneMap = Arc<RwLock<HashMap<String, DroneEntry>>>;
pub type SharedKey = Arc<QueenKey>;

/// Command context — holds shared state for the Queen
pub struct QueenContext {
    pub queen_key: SharedKey,
    pub drones: DroneMap,
    pub nonce_window: Arc<RwLock<NonceWindow>>,
    pub socket: Arc<UdpSocket>,
    pub registry_path: String,
}

impl QueenContext {
    pub async fn new(socket: Arc<UdpSocket>, queen_key: SharedKey, registry_path: String) -> Self {
        let drones = Arc::new(RwLock::new(HashMap::new()));
        let nonce_window = Arc::new(RwLock::new(NonceWindow::new(10000)));

        // Try to load existing registry
        if let Ok(data) = std::fs::read_to_string(&registry_path) {
            if let Ok(registry) = serde_json::from_str::<HashMap<String, DroneEntry>>(&data) {
                let mut d = drones.write().await;
                *d = registry;
                eprintln!("[QUEEN] Loaded {} drones from registry", d.len());
            }
        }

        Self {
            queen_key,
            drones,
            nonce_window,
            socket,
            registry_path,
        }
    }

    /// Save registry to disk
    pub async fn save_registry(&self) -> std::io::Result<()> {
        let drones = self.drones.read().await;
        let json = serde_json::to_string_pretty(&*drones)?;
        std::fs::write(&self.registry_path, json)?;
        Ok(())
    }
}

// ── Main dispatcher ──

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let state_dir = cli.state_dir.clone();
    std::fs::create_dir_all(&state_dir).map_err(|e| e.to_string())?;

    let queen_key = QueenKey::load_or_create(&state_dir).map_err(|e| e.to_string())?;
    let queen_key = Arc::new(queen_key);

    match cli.command {
        Some(Commands::Start { port }) => {
            let port = port.unwrap_or(cli.port);
            Box::pin(start_server(port, &state_dir, queen_key)).await
                .map_err(|e| e.to_string())?;
            Ok("Server started".to_string())
        }
        Some(Commands::Status { name }) => {
            Box::pin(query_status(&state_dir, name.as_deref())).await
        }
        Some(Commands::RunAll { task }) => {
            Box::pin(run_all(&state_dir, &task, queen_key)).await
        }
        Some(Commands::Apoptosis) => {
            Box::pin(send_apoptosis(&state_dir, queen_key)).await
        }
        Some(Commands::KeyInfo) => {
            output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
            output::section("Queen Key");
            output::kv("Status", "✓ Active");
            output::kv("Algorithm", "BLAKE3-keyed");
            output::kv("Key Size", "256 bits");
            output::kv("Storage", &format!("{}/{}", state_dir, ".bio-queen.key"));
            output::summary("omega-master", "Queen authentication ready");
            Ok("".to_string())
        }
        Some(Commands::Freeze) => {
            Box::pin(freeze_system(&state_dir, queen_key)).await
        }
        Some(Commands::Thaw) => {
            Box::pin(thaw_system(&state_dir, queen_key)).await
        }
        Some(Commands::Microscope { query, limit }) => {
            output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
            output::section("Microscope Query");
            output::kv("Query", &query);
            output::kv("Limit", &limit.to_string());
            output::kv("Status", "Not yet implemented in Queen");
            output::summary("omega-master", "Microscope query");
            Ok("".to_string())
        }
        Some(Commands::Homeo) => {
            output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
            output::section("Homeostasis");
            output::kv("Trigger", "Manual");
            output::kv("Status", "Homeostasis loop runs asynchronously");
            output::summary("omega-master", "Homeostasis evaluation triggered");
            Ok("".to_string())
        }
        None => {
            output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
            output::section("Commands");
            println!("  start           Start Queen server");
            println!("  status          Query drone status");
            println!("  run-all         Run task on all drones");
            println!("  apoptosis       Emergency signal");
            println!("  key-info        Show Queen key");
            println!("  freeze          Freeze all drones");
            println!("  thaw            Thaw all drones");
            println!("  microscope      Query memory");
            println!("  homeo           Trigger homeostasis");
            output::summary("omega-master", "Queen Server ready");
            Ok("".to_string())
        }
    }
}

// ── Server startup ──

async fn start_server(port: u16, state_dir: &str, queen_key: SharedKey) -> Result<(), Box<dyn std::error::Error>> {
    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");

    let addr = format!("0.0.0.0:{}", port);
    let socket = UdpSocket::bind(&addr).await?;
    let socket = Arc::new(socket);

    output::section("Configuration");
    output::kv("Bind Address", &addr);
    output::kv("State Directory", state_dir);
    output::kv("Protocol", "v2: BioMessage (Binary)");
    output::kv("Auth", "BLAKE3 keyed hash");
    println!();

    let registry_path = format!("{}/.bio-drones.json", state_dir);
    let context = QueenContext::new(socket.clone(), queen_key.clone(), registry_path).await;
    let context = Arc::new(context);

    output::section("Queen Server Online");
    output::success("Listening for drone joins...");
    println!();

    // Spawn homeostasis loop
    let homeo_drones = context.drones.clone();
    let _homeo_key = queen_key.clone();
    let _homeo_socket = socket.clone();
    tokio::spawn(async move {
        homeostasis_loop(homeo_drones).await;
    });

    // Spawn thermal sensor loop
    let thermal_drones = context.drones.clone();
    let _thermal_key = queen_key.clone();
    let _thermal_socket = socket.clone();
    tokio::spawn(async move {
        thermal_sensor_loop(thermal_drones).await;
    });

    // Main message loop
    let mut buf = [0u8; 65535];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                let context = context.clone();
                let data = buf[..len].to_vec();

                tokio::spawn(async move {
                    if let Err(e) = handle_message(&context, &data, addr).await {
                        eprintln!("[QUEEN] Error handling message from {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("[QUEEN] Recv error: {}", e);
            }
        }
    }
}

// ── Message handling ──

async fn handle_message(ctx: &QueenContext, data: &[u8], addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    // Try v2 BioMessage first
    if let Ok(msg) = BioMessage::decode(data) {
        return handle_bio_message(ctx, &msg, addr).await;
    }

    eprintln!("[QUEEN] Unknown message format from {}", addr);
    Ok(())
}

async fn handle_bio_message(ctx: &QueenContext, msg: &BioMessage, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    // Verify auth only for non-Join messages
    if msg.op != BioOp::Join && !ctx.queen_key.verify(msg) {
        return Err("Auth verification failed".into());
    }

    // Check replay
    let mut nonce_win = ctx.nonce_window.write().await;
    if !nonce_win.check_and_insert(msg.nonce) {
        return Err(format!("Replay detected: nonce {}", msg.nonce).into());
    }
    drop(nonce_win);

    match msg.op {
        BioOp::Join => {
            handle_join(ctx, msg, addr).await?;
        }
        BioOp::Heartbeat => {
            handle_heartbeat(ctx, msg, addr).await?;
        }
        BioOp::Result => {
            handle_result(ctx, msg, addr).await?;
        }
        BioOp::Status => {
            handle_status_query(ctx, msg, addr).await?;
        }
        BioOp::FrozenAck => {
            handle_frozen_ack(ctx, msg, addr).await?;
        }
        _ => {
            eprintln!("[QUEEN] Unhandled opcode: {:?}", msg.op);
        }
    }

    Ok(())
}

async fn handle_join(ctx: &QueenContext, msg: &BioMessage, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let fields = bio_protocol::decode_fields(&msg.payload);
    let name = fields.iter()
        .find(|(k, _)| k == "name")
        .and_then(|(_, v)| std::str::from_utf8(v).ok())
        .unwrap_or("unknown");

    let mut drones = ctx.drones.write().await;
    drones.insert(
        name.to_string(),
        DroneEntry {
            name: name.to_string(),
            addr: addr.to_string(),
            generation: msg.generation,
            status: "alive".to_string(),
            protocol: "v2-bio".to_string(),
            last_heartbeat: now_ms(),
            joined_at: now_ms(),
        },
    );
    drop(drones);

    eprintln!("[QUEEN] Drone joined: {} (addr={}, gen={})", name, addr, msg.generation);

    // Save registry
    ctx.save_registry().await?;

    Ok(())
}

async fn handle_heartbeat(ctx: &QueenContext, msg: &BioMessage, _addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let fields = bio_protocol::decode_fields(&msg.payload);
    let name = fields.iter()
        .find(|(k, _)| k == "name")
        .and_then(|(_, v)| std::str::from_utf8(v).ok())
        .unwrap_or("unknown");

    let mut drones = ctx.drones.write().await;
    if let Some(entry) = drones.get_mut(name) {
        entry.last_heartbeat = now_ms();
    }
    drop(drones);

    Ok(())
}

async fn handle_result(_ctx: &QueenContext, msg: &BioMessage, _addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let fields = bio_protocol::decode_fields(&msg.payload);

    if let Some((_, name_bytes)) = fields.iter().find(|(k, _)| k == "name") {
        let name = std::str::from_utf8(name_bytes).unwrap_or("unknown");
        if let Some((_, result_bytes)) = fields.iter().find(|(k, _)| k == "result") {
            let result_str = std::str::from_utf8(result_bytes).unwrap_or("<binary>");
            eprintln!("[QUEEN] Result from {}: {}", name, result_str);
        }
    }

    Ok(())
}

async fn handle_status_query(ctx: &QueenContext, _msg: &BioMessage, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let drones = ctx.drones.read().await;
    for entry in drones.values() {
        if entry.addr == addr.to_string() {
            let payload = bio_protocol::encode_fields(&[
                ("name", entry.name.as_bytes()),
                ("status", entry.status.as_bytes()),
                ("generation", &entry.generation.to_le_bytes()),
            ]);
            let mut response = BioMessage::new(BioOp::Status, entry.generation, payload);
            ctx.queen_key.sign(&mut response);
            let packet = response.encode();
            let _ = ctx.socket.send_to(&packet, addr).await;
            break;
        }
    }
    Ok(())
}

async fn handle_frozen_ack(_ctx: &QueenContext, msg: &BioMessage, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let fields = bio_protocol::decode_fields(&msg.payload);
    let name = fields.iter()
        .find(|(k, _)| k == "name")
        .and_then(|(_, v)| std::str::from_utf8(v).ok())
        .unwrap_or("unknown");

    eprintln!("[QUEEN] Drone frozen: {} (addr={})", name, addr);
    Ok(())
}

// ── Helper functions ──

async fn query_status(state_dir: &str, name_filter: Option<&str>) -> Result<String, String> {
    let registry_path = format!("{}/.bio-drones.json", state_dir);

    let drones: HashMap<String, DroneEntry> = if std::path::Path::new(&registry_path).exists() {
        let data = std::fs::read_to_string(&registry_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    } else {
        HashMap::new()
    };

    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
    output::section("Drone Status");

    if drones.is_empty() {
        output::warn("No drones registered");
    } else {
        for entry in drones.values() {
            if let Some(filter) = name_filter {
                if !entry.name.contains(filter) { continue; }
            }

            let status_color = match entry.status.as_str() {
                "alive" => entry.status.bright_green(),
                "frozen" => entry.status.bright_yellow(),
                _ => entry.status.normal(),
            };

            println!("    {} [{}] gen={} proto={}",
                entry.name.cyan(),
                status_color,
                entry.generation,
                entry.protocol
            );
            println!("      addr: {}", entry.addr.dimmed());
            println!("      joined: {}", format_time(entry.joined_at));
            println!("      last_hb: {}", format_time(entry.last_heartbeat));
        }
    }

    output::summary("omega-master", &format!("{} drones", drones.len()));
    Ok("".to_string())
}

async fn run_all(state_dir: &str, task: &str, queen_key: SharedKey) -> Result<String, String> {
    let registry_path = format!("{}/.bio-drones.json", state_dir);
    let drones: HashMap<String, DroneEntry> = if std::path::Path::new(&registry_path).exists() {
        let data = std::fs::read_to_string(&registry_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    } else {
        HashMap::new()
    };

    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
    output::section("Broadcasting Task");
    output::kv("Task", task);
    output::kv("Target Drones", &drones.len().to_string());

    if drones.is_empty() {
        output::warn("No drones online");
        return Ok("".to_string());
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;

    for entry in drones.values() {
        if entry.protocol == "v2-bio" {
            if let Ok(addr) = entry.addr.parse::<SocketAddr>() {
                let payload = bio_protocol::encode_fields(&[
                    ("task", task.as_bytes()),
                ]);
                let mut msg = BioMessage::new(BioOp::Task, entry.generation, payload);
                queen_key.sign(&mut msg);
                let packet = msg.encode();
                let _ = socket.send_to(&packet, addr).await;

                output::success(&format!("Task sent to {}", entry.name));
            }
        }
    }

    output::summary("omega-master", "Tasks broadcast");
    Ok("".to_string())
}

async fn send_apoptosis(state_dir: &str, queen_key: SharedKey) -> Result<String, String> {
    let registry_path = format!("{}/.bio-drones.json", state_dir);
    let drones: HashMap<String, DroneEntry> = if std::path::Path::new(&registry_path).exists() {
        let data = std::fs::read_to_string(&registry_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    } else {
        HashMap::new()
    };

    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
    output::section("APOPTOSIS");
    output::error("SENDING TERMINATION SIGNAL TO ALL DRONES");
    output::kv("Target Count", &drones.len().to_string());

    if drones.is_empty() {
        output::warn("No drones to terminate");
        return Ok("".to_string());
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;

    for entry in drones.values() {
        if entry.protocol == "v2-bio" {
            if let Ok(addr) = entry.addr.parse::<SocketAddr>() {
                let mut msg = BioMessage::new(BioOp::Apoptosis, entry.generation, vec![]);
                queen_key.sign(&mut msg);
                let packet = msg.encode();
                let _ = socket.send_to(&packet, addr).await;

                output::success(&format!("Apoptosis signal sent to {}", entry.name));
            }
        }
    }

    output::summary("omega-master", "APOPTOSIS COMPLETE");
    Ok("".to_string())
}

async fn freeze_system(state_dir: &str, queen_key: SharedKey) -> Result<String, String> {
    let registry_path = format!("{}/.bio-drones.json", state_dir);
    let mut drones: HashMap<String, DroneEntry> = if std::path::Path::new(&registry_path).exists() {
        let data = std::fs::read_to_string(&registry_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    } else {
        HashMap::new()
    };

    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
    output::section("System Freeze");
    output::kv("Target Drones", &drones.len().to_string());

    if drones.is_empty() {
        output::warn("No drones to freeze");
        return Ok("".to_string());
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;

    for entry in drones.values_mut() {
        if entry.protocol == "v2-bio" {
            if let Ok(addr) = entry.addr.parse::<SocketAddr>() {
                let mut msg = BioMessage::new(BioOp::Freeze, entry.generation, vec![]);
                queen_key.sign(&mut msg);
                let packet = msg.encode();
                let _ = socket.send_to(&packet, addr).await;

                entry.status = "frozen".to_string();
                output::success(&format!("Freeze signal sent to {}", entry.name));
            }
        }
    }

    // Save updated registry
    let json = serde_json::to_string_pretty(&drones).map_err(|e| e.to_string())?;
    std::fs::write(&registry_path, json).map_err(|e| e.to_string())?;

    output::summary("omega-master", "System frozen");
    Ok("".to_string())
}

async fn thaw_system(state_dir: &str, queen_key: SharedKey) -> Result<String, String> {
    let registry_path = format!("{}/.bio-drones.json", state_dir);
    let mut drones: HashMap<String, DroneEntry> = if std::path::Path::new(&registry_path).exists() {
        let data = std::fs::read_to_string(&registry_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    } else {
        HashMap::new()
    };

    output::banner("OMEGA-MASTER", "Queen Server (v2: DNA Protocol)", "👑");
    output::section("System Thaw");
    output::kv("Target Drones", &drones.len().to_string());

    if drones.is_empty() {
        output::warn("No drones to thaw");
        return Ok("".to_string());
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;

    for entry in drones.values_mut() {
        if entry.protocol == "v2-bio" {
            if let Ok(addr) = entry.addr.parse::<SocketAddr>() {
                let mut msg = BioMessage::new(BioOp::Thaw, entry.generation, vec![]);
                queen_key.sign(&mut msg);
                let packet = msg.encode();
                let _ = socket.send_to(&packet, addr).await;

                entry.status = "alive".to_string();
                output::success(&format!("Thaw signal sent to {}", entry.name));
            }
        }
    }

    // Save updated registry
    let json = serde_json::to_string_pretty(&drones).map_err(|e| e.to_string())?;
    std::fs::write(&registry_path, json).map_err(|e| e.to_string())?;

    output::summary("omega-master", "System thawed");
    Ok("".to_string())
}

// ── Background loops ──

async fn homeostasis_loop(drones: DroneMap) {
    eprintln!("[QUEEN-HOMEO] Homeostasis loop started");

    let mut tick = 0u64;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        tick += 1;

        let drones_read = drones.read().await;
        let online_count = drones_read.len();
        drop(drones_read);

        if online_count > 0 && tick % 3 == 0 {
            eprintln!("[QUEEN-HOMEO] Homeostasis tick #{} ({} drones)", tick, online_count);
        }
    }
}

async fn thermal_sensor_loop(drones: DroneMap) {
    eprintln!("[QUEEN-THERMAL] Thermal sensor loop started");

    let mut tick = 0u64;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        tick += 1;

        let drones_read = drones.read().await;
        for entry in drones_read.values() {
            if tick % 6 == 0 {
                eprintln!("[QUEEN-THERMAL] Monitoring {}: {} (last_hb={}ms ago)",
                    entry.name,
                    entry.status,
                    now_ms() - entry.last_heartbeat
                );
            }
        }
        drop(drones_read);
    }
}

// ── Formatting helpers ──

fn format_time(ts: u64) -> String {
    let now = now_ms();
    if ts > now {
        "in future".to_string()
    } else {
        let diff_ms = now - ts;
        if diff_ms < 1000 {
            format!("{}ms ago", diff_ms)
        } else if diff_ms < 60000 {
            format!("{}s ago", diff_ms / 1000)
        } else {
            format!("{}m ago", diff_ms / 60000)
        }
    }
}
