/// ThermalSensor — CPU Temperature → WaveField FEVER pulse.
///
/// Reads system CPU temperature using `sysinfo` and injects a FEVER wave
/// into the WaveField. High temps = high amplitude = the organism is in distress.
/// The Ítélőszék (LLM homeostasis) reads this as part of the bio-state context.

use sysinfo::Components;
use std::time::Duration;
use crate::wave_store::{WaveStore, WavePacket, WaveOrigin, channels, now_ms, default_path};

/// Normal operating temperature threshold in °C.
const NORMAL_TEMP_C: f32 = 60.0;
/// Critical temperature threshold in °C — organism is in FEVER.
const FEVER_TEMP_C: f32 = 80.0;
/// Maximum expected temperature for amplitude normalization.
const MAX_TEMP_C: f32 = 100.0;

/// Run the thermal sensing loop, injecting FEVER waves into WaveField.
pub async fn run_thermal_loop() {
    eprintln!("[THERMAL] Starting CPU thermal monitor loop...");

    let mut id_counter: u64 = 90000; // Thermal packet IDs start at 90000

    loop {
        let cpu_temp = read_cpu_temp();

        if let Some(temp) = cpu_temp {
            let amplitude = temp_to_amplitude(temp);
            let decay = if temp >= FEVER_TEMP_C { 0.0002 } else { 0.001 };

            if amplitude > 0.1 {
                // Inject into WaveStore
                if let Ok(mut store) = WaveStore::load(&default_path(), 10000) {
                    id_counter += 1;
                    let packet = WavePacket {
                        id: id_counter,
                        emitted_at: now_ms(),
                        frequency: channels::FEVER,
                        amplitude,
                        decay,
                        phase: 0.0,
                        origin: WaveOrigin::FieldEmergence,
                        tag: Some(format!("cpu_temp={:.1}C", temp)),
                    };
                    store.emit(packet);
                    store.persist().ok();

                    let status = if temp >= FEVER_TEMP_C {
                        "🔴 FEVER"
                    } else if temp >= NORMAL_TEMP_C {
                        "🟠 WARM"
                    } else {
                        "🟢 COOL"
                    };

                    eprintln!(
                        "[THERMAL] {} CPU: {:.1}C → FEVER wave amplitude={:.2} decay={:.4}",
                        status, temp, amplitude, decay
                    );
                }
            } else {
                eprintln!("[THERMAL] 🟢 COOL CPU: {:.1}C — no FEVER wave needed.", temp);
            }
        } else {
            eprintln!("[THERMAL] No temperature sensors found (virtualized?). Skipping tick.");
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Read average CPU temperature from sysinfo components.
fn read_cpu_temp() -> Option<f32> {
    let components = Components::new_with_refreshed_list();

    // Filter CPU-related components
    let cpu_temps: Vec<f32> = components
        .iter()
        .filter(|c| {
            let label = c.label().to_lowercase();
            label.contains("cpu") || label.contains("core") || label.contains("package")
        })
        .map(|c| c.temperature())
        .collect();

    if !cpu_temps.is_empty() {
        // Take the max CPU temperature (hottest core)
        cpu_temps.into_iter().reduce(f32::max)
    } else {
        // Fallback: average all components
        let all: Vec<f32> = components.iter().map(|c| c.temperature()).collect();
        if all.is_empty() {
            return None;
        }
        Some(all.iter().sum::<f32>() / all.len() as f32)
    }
}

/// Convert temperature in °C to wave amplitude (0.0 - 1.5).
fn temp_to_amplitude(temp: f32) -> f32 {
    // Below normal: near-zero amplitude (organism is cool/healthy)
    // Above FEVER: amplitude > 1.0 (constructive interference triggers Ítélőszék)
    ((temp - NORMAL_TEMP_C) / (MAX_TEMP_C - NORMAL_TEMP_C)).max(0.0) * 1.5
}

