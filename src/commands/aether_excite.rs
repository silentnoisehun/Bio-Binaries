use crate::bio_client;
use clap::Parser;
use serde::Serialize;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

#[derive(Parser)]
#[command(name = "aether-excite", about = "Resource excitation monitor — per-region system load")]
pub struct Cli {

    /// Echo-X master address
    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExcitationRegion {
    pub name: String,
    pub region_type: String,
    pub usage_percent: f64,
    pub excitation_level: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ExcitationResult {
    pub timestamp: String,
    pub regions: Vec<ExcitationRegion>,
    pub gravity_wells: Vec<String>,
    pub total_excitation: f64,
    pub field_status: String,
}

pub fn excitation_level(pct: f64) -> &'static str {
    if pct > 90.0 { "CRITICAL" }
    else if pct > 80.0 { "EXTREME" }
    else if pct > 60.0 { "HIGH" }
    else if pct > 40.0 { "MODERATE" }
    else if pct > 20.0 { "LOW" }
    else { "DORMANT" }
}

pub fn run() -> ExcitationResult {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    sys.refresh_cpu();
    sys.refresh_memory();

    let mut regions = Vec::new();
    let mut gravity_wells = Vec::new();

    // Kernel_Core: per-CPU excitation
    for (i, cpu) in sys.cpus().iter().enumerate() {
        let usage = cpu.cpu_usage() as f64;
        let level = excitation_level(usage);
        if usage > 80.0 {
            gravity_wells.push(format!("CPU_{} ({:.1}%)", i, usage));
        }
        regions.push(ExcitationRegion {
            name: format!("Kernel_Core_{}", i),
            region_type: "CPU".into(),
            usage_percent: (usage * 10.0).round() / 10.0,
            excitation_level: level.into(),
            details: serde_json::json!({
                "frequency_mhz": cpu.frequency(),
                "name": cpu.name(),
            }),
        });
    }

    // Memory_Pool
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let mem_pct = if total_mem > 0 { (used_mem as f64 / total_mem as f64) * 100.0 } else { 0.0 };
    let mem_level = excitation_level(mem_pct);
    if mem_pct > 80.0 {
        gravity_wells.push(format!("Memory_Pool ({:.1}%)", mem_pct));
    }
    regions.push(ExcitationRegion {
        name: "Memory_Pool".into(),
        region_type: "RAM".into(),
        usage_percent: (mem_pct * 10.0).round() / 10.0,
        excitation_level: mem_level.into(),
        details: serde_json::json!({
            "total_mb": total_mem / (1024 * 1024),
            "used_mb": used_mem / (1024 * 1024),
            "swap_total_mb": sys.total_swap() / (1024 * 1024),
            "swap_used_mb": sys.used_swap() / (1024 * 1024),
        }),
    });

    // IO_Gateway: per-disk
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.iter() {
        let total = disk.total_space() as f64;
        let avail = disk.available_space() as f64;
        let pct = if total > 0.0 { ((total - avail) / total) * 100.0 } else { 0.0 };
        let level = excitation_level(pct);
        let mp = disk.mount_point().to_string_lossy().to_string();
        if pct > 80.0 {
            gravity_wells.push(format!("IO_Gateway_{} ({:.1}%)", mp, pct));
        }
        regions.push(ExcitationRegion {
            name: format!("IO_Gateway_{}", mp),
            region_type: "Disk".into(),
            usage_percent: (pct * 10.0).round() / 10.0,
            excitation_level: level.into(),
            details: serde_json::json!({
                "total_gb": (total / 1073741824.0 * 100.0).round() / 100.0,
                "available_gb": (avail / 1073741824.0 * 100.0).round() / 100.0,
                "filesystem": format!("{:?}", disk.file_system()),
            }),
        });
    }

    // Skill_Storage: key directories
    let skill_dirs = [
        ("D:\\All_Skills", "Skill_Storage_AllSkills"),
        ("D:\\Future write in present", "Skill_Storage_FutureWrite"),
    ];
    for (path, name) in &skill_dirs {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_dir() {
                let count = std::fs::read_dir(path).map(|rd| rd.count()).unwrap_or(0);
                regions.push(ExcitationRegion {
                    name: name.to_string(),
                    region_type: "Directory".into(),
                    usage_percent: 0.0,
                    excitation_level: "MAPPED".into(),
                    details: serde_json::json!({
                        "path": path,
                        "entries": count,
                    }),
                });
            }
        }
    }

    let total_excitation: f64 = regions.iter()
        .filter(|r| r.region_type != "Directory")
        .map(|r| r.usage_percent)
        .sum::<f64>()
        / regions.iter().filter(|r| r.region_type != "Directory").count().max(1) as f64;

    let field_status = if gravity_wells.is_empty() {
        "STABLE"
    } else if gravity_wells.len() <= 2 {
        "TURBULENT"
    } else {
        "CRITICAL"
    };

    ExcitationResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        regions,
        gravity_wells,
        total_excitation: (total_excitation * 10.0).round() / 10.0,
        field_status: field_status.into(),
    }
}

fn format_pretty(result: &ExcitationResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  ⚡ AETHER-EXCITE \n"));
    out.push_str(&format!("║  Layer: Quantum-Space / Excitation Monitor\n"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    out.push_str(&format!("  ▸ Kernel Cores (CPU)\n"));
    for r in result.regions.iter().filter(|r| r.region_type == "CPU") {
        out.push_str(&format!("    {} {:.1}% [{}]\n", r.name, r.usage_percent, r.excitation_level));
    }

    out.push_str("\n");
    out.push_str(&format!("  ▸ Memory Pool\n"));
    for r in result.regions.iter().filter(|r| r.region_type == "RAM") {
        let pct = (r.usage_percent).min(100.0);
        let filled = (pct / 5.0) as usize;
        let empty = 20 - filled;
        out.push_str(&format!("    {} [{}{}] {:.1}%\n",
            r.name, "█".repeat(filled), "░".repeat(empty), pct));
    }

    out.push_str("\n");
    out.push_str(&format!("  ▸ IO Gateways (Disk)\n"));
    for r in result.regions.iter().filter(|r| r.region_type == "Disk") {
        let pct = (r.usage_percent).min(100.0);
        let filled = (pct / 5.0) as usize;
        let empty = 20 - filled;
        out.push_str(&format!("    {} [{}{}] {:.1}%\n",
            r.name, "█".repeat(filled), "░".repeat(empty), pct));
    }

    if !result.gravity_wells.is_empty() {
        out.push_str("\n");
        out.push_str(&format!("  ▸ Gravity Wells (>80%)\n"));
        for gw in &result.gravity_wells {
            out.push_str(&format!("  [WARN] {}\n", gw));
        }
    }

    out.push_str("\n");
    out.push_str(&format!("    Total Excitation: {:.1}%\n", result.total_excitation));
    out.push_str(&format!("\n  ⟫ aether-excite :: {}\n\n", result.field_status));

    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    let result = run();

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("aether-excite", addr).await {
            let excitation_str = format!("{:.2}", result.total_excitation);
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("total_excitation", excitation_str.as_bytes()),
                ("field_status", result.field_status.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
