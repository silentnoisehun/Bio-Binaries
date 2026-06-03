use serde::Serialize;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize)]
pub struct CpuInfo {
    pub name: String,
    pub usage_percent: f32,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub timestamp: String,
    pub hostname: String,
    pub cpus: Vec<CpuInfo>,
    pub cpu_global_usage: f32,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub process_count: usize,
}

pub fn take_snapshot() -> SystemSnapshot {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    // Need two refreshes for accurate CPU readings
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu();
    sys.refresh_memory();

    let cpus: Vec<CpuInfo> = sys.cpus().iter().map(|cpu| CpuInfo {
        name: cpu.name().to_string(),
        usage_percent: cpu.cpu_usage(),
        frequency_mhz: cpu.frequency(),
    }).collect();

    let cpu_global = sys.global_cpu_info().cpu_usage();

    let total_mem = sys.total_memory() / (1024 * 1024);
    let used_mem = sys.used_memory() / (1024 * 1024);
    let avail_mem = sys.available_memory() / (1024 * 1024);
    let mem_pct = if total_mem > 0 { (used_mem as f64 / total_mem as f64) * 100.0 } else { 0.0 };

    let disks_obj = Disks::new_with_refreshed_list();
    let disks: Vec<DiskInfo> = disks_obj.iter().map(|d| {
        let total = d.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);
        let avail = d.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
        let pct = if total > 0.0 { ((total - avail) / total) * 100.0 } else { 0.0 };
        DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            mount_point: d.mount_point().to_string_lossy().to_string(),
            total_gb: (total * 100.0).round() / 100.0,
            available_gb: (avail * 100.0).round() / 100.0,
            usage_percent: (pct * 10.0).round() / 10.0,
        }
    }).collect();

    let process_count = {
        let mut sys2 = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
        );
        sys2.refresh_processes();
        sys2.processes().len()
    };

    SystemSnapshot {
        timestamp: chrono::Utc::now().to_rfc3339(),
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        cpus,
        cpu_global_usage: cpu_global,
        memory: MemoryInfo {
            total_mb: total_mem,
            used_mb: used_mem,
            available_mb: avail_mem,
            usage_percent: (mem_pct * 10.0).round() / 10.0,
        },
        disks,
        process_count,
    }
}

pub fn get_top_processes(limit: usize) -> Vec<ProcessInfo> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_processes();

    let mut procs: Vec<ProcessInfo> = sys.processes().iter().map(|(pid, p)| {
        ProcessInfo {
            pid: pid.as_u32(),
            name: p.name().to_string(),
            cpu_usage: p.cpu_usage(),
            memory_mb: p.memory() / (1024 * 1024),
        }
    }).collect();

    procs.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
    procs.truncate(limit);
    procs
}
