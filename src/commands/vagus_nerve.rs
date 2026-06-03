/// vagus-nerve — The internal sensing nerve of the ORA organism.
///
/// Like the biological vagus nerve, this binary is the feedback loop between
/// the body (CPU, RAM, disk) and the brain (wave-field). It reads system vitals
/// and injects them as waves into the WaveStore, so the field can feel
/// what is happening inside the machine.
///
/// Channels:
///   - CPU_LOAD (12Hz): normalized CPU usage → amplitude
///   - SYSTEM_HEALTH (4Hz): overall health pulse (inverse of stress)
///   - FEVER (37Hz): delegates to thermal_sensor for temperature

use clap::Parser;
use sysinfo::System;
use std::time::Duration;

use crate::output;
use crate::wave_store::{self, WaveStore, WavePacket, WaveOrigin, channels};

#[derive(Parser)]
#[command(name = "vagus-nerve", about = "Internal organ sensing — CPU/RAM → WaveField")]
pub struct Cli {
    /// Sensing interval in milliseconds
    #[arg(long, default_value = "2000")]
    pub interval: u64,

    /// Show continuous output
    #[arg(long)]
    pub watch: bool,

    /// Single snapshot mode (default if no --watch)
    #[arg(long)]
    pub snapshot: bool,
}

/// Convert CPU usage (0-100%) to wave amplitude (0.0-1.0).
/// Below 30% = negligible. Above 85% = approaching saturation.
fn cpu_to_amplitude(cpu_percent: f32) -> f32 {
    if cpu_percent < 10.0 {
        0.0 // idle — no signal needed
    } else {
        // Normalize: 10% → 0.05, 50% → 0.4, 85% → 0.75, 100% → 1.0
        ((cpu_percent - 10.0) / 90.0).min(1.0).max(0.0)
    }
}

/// Convert RAM usage (0-100%) to a health modifier.
/// High RAM = less healthy → negative health contribution.
fn ram_to_health_modifier(ram_percent: f32) -> f32 {
    if ram_percent < 60.0 {
        0.0 // plenty of RAM — no stress
    } else {
        // 60% → 0.0, 80% → -0.2, 95% → -0.35
        -((ram_percent - 60.0) / 100.0).min(0.5)
    }
}

fn sense_once(sys: &mut System) -> (f32, f32, f32) {
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
        / sys.cpus().len().max(1) as f32;

    let total_mem = sys.total_memory() as f64;
    let used_mem = sys.used_memory() as f64;
    let ram_percent = if total_mem > 0.0 {
        (used_mem / total_mem * 100.0) as f32
    } else {
        0.0
    };

    let cpu_amp = cpu_to_amplitude(cpu_usage);
    let ram_mod = ram_to_health_modifier(ram_percent);

    (cpu_usage, ram_percent, cpu_amp + ram_mod.abs()) // combined stress
}

fn inject_vitals(cpu_usage: f32, ram_percent: f32) {
    let store_path = wave_store::default_path();
    let now = wave_store::now_ms();

    let cpu_amp = cpu_to_amplitude(cpu_usage);
    let ram_mod = ram_to_health_modifier(ram_percent);

    // ── CPU_LOAD (12Hz) ──
    // Only inject if there's meaningful load
    if cpu_amp > 0.05 {
        let cpu_wave = WavePacket {
            emitted_at: now,
            frequency: channels::CPU_LOAD,
            amplitude: cpu_amp,
            decay: 0.002, // ~500ms half-life — CPU state changes fast
            origin: WaveOrigin::VagusNerve,
            tag: Some(format!("cpu={:.1}%", cpu_usage)),
            ..Default::default()
        };
        let _ = WaveStore::append_to_inbox(&store_path, &cpu_wave);
    }

    // ── SYSTEM_HEALTH (4Hz) ──
    // Health is the inverse of stress: low CPU + low RAM = strong health pulse
    // High CPU or high RAM = weakened or negative health
    let health_amp = 0.1 + ram_mod; // base 0.1, reduced by RAM pressure
    if health_amp.abs() > 0.01 {
        let health_wave = WavePacket {
            emitted_at: now,
            frequency: channels::SYSTEM_HEALTH,
            amplitude: health_amp,
            decay: 0.001, // ~1s half-life — health pulses rhythmically
            origin: WaveOrigin::VagusNerve,
            tag: Some(format!("health ram={:.0}%", ram_percent)),
            ..Default::default()
        };
        let _ = WaveStore::append_to_inbox(&store_path, &health_wave);
    }
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    output::banner("VAGUS-NERVE", "Internal Organ Sensing", "");

    let mut sys = System::new();
    // Initial refresh to populate CPU data (first call returns 0)
    sys.refresh_cpu_usage();
    tokio::time::sleep(Duration::from_millis(500)).await;

    if cli.watch {
        output::section("Continuous Sensing");
        output::kv("Interval", &format!("{}ms", cli.interval));
        output::kv("Channels", "CPU_LOAD(12Hz), SYSTEM_HEALTH(4Hz)");

        eprintln!("[VAGUS] Sensing loop started (interval={}ms)", cli.interval);
        eprintln!("[VAGUS] Injecting into: {:?}", wave_store::default_path());

        let mut tick: u64 = 0;
        loop {
            let (cpu, ram, _stress) = sense_once(&mut sys);
            inject_vitals(cpu, ram);

            let cpu_amp = cpu_to_amplitude(cpu);
            let status = if cpu_amp > 0.7 {
                "HIGH"
            } else if cpu_amp > 0.3 {
                "WARM"
            } else {
                "CALM"
            };

            if tick % 5 == 0 {
                eprintln!(
                    "[VAGUS] Tick {}: CPU={:.1}% ({}) RAM={:.1}% → 12Hz amp={:.2}, 4Hz health={:.2}",
                    tick, cpu, status, ram, cpu_amp, 0.1 + ram_to_health_modifier(ram)
                );
            }

            tick += 1;
            tokio::time::sleep(Duration::from_millis(cli.interval)).await;
        }
    } else {
        // Single snapshot
        let (cpu, ram, stress) = sense_once(&mut sys);
        inject_vitals(cpu, ram);

        output::section("System Vitals");
        output::kv("CPU", &format!("{:.1}%", cpu));
        output::kv("RAM", &format!("{:.1}%", ram));
        output::kv("CPU→12Hz", &format!("amp={:.3}", cpu_to_amplitude(cpu)));
        output::kv("RAM→Health", &format!("mod={:.3}", ram_to_health_modifier(ram)));
        output::kv("Combined Stress", &format!("{:.3}", stress));
        output::summary("vagus-nerve", "vitals injected into wave field");
    }

    Ok(String::new())
}
