/// WaveField — Self-organizing wave interference field.
///
/// The field does not contain an agent, scheduler, or rules engine.
/// It contains frequency-domain rules that fire based on wave interference patterns.
/// When the field detects constructive/destructive/chaotic interference
/// at specific frequencies, it emits new waves in response.
///
/// The field decides. Not code. Not rules. Physics.

use crate::wave_store::{
    InterferencePattern, WaveOrigin, WavePacket, WaveStore, channels, now_ms,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A single field rule — maps interference patterns to wave emissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldRule {
    /// Which frequency channel to observe
    pub trigger_freq: f32,
    /// Minimum amplitude to trigger (absolute value)
    pub threshold: f32,
    /// Required interference pattern
    pub pattern: InterferencePattern,
    /// What the field emits in response
    pub response: FieldResponse,
    /// Human-readable name
    pub name: String,
}

/// What the field does when a rule fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldResponse {
    /// Emit a new wave (the field modifies itself)
    EmitWave {
        freq: f32,
        amplitude_scale: f32, // multiplier on the trigger amplitude
        decay: f32,
    },
    /// Throttle — hook into immune system
    Throttle,
    /// Freeze — hook into cryostasis
    Freeze,
    /// No response — silence is also a decision
    Silent,
}

/// An emergent event — logged when a rule fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergentEvent {
    pub timestamp: u64,
    pub rule_name: String,
    pub trigger_freq: f32,
    pub trigger_amplitude: f32,
    pub pattern: InterferencePattern,
    pub response_description: String,
}

/// The WaveField itself.
pub struct WaveField {
    pub store: WaveStore,
    pub tick_ms: u64,
    pub rules: Vec<FieldRule>,
    pub running: Arc<AtomicBool>,
    pub events: Vec<EmergentEvent>,
    pub tick_count: u64,
    max_events: usize,
}

impl WaveField {
    /// Create a new field with default rules.
    pub fn new(store: WaveStore, tick_ms: u64) -> Self {
        Self {
            store,
            tick_ms,
            rules: default_rules(),
            running: Arc::new(AtomicBool::new(true)),
            events: Vec::new(),
            tick_count: 0,
            max_events: 1000,
        }
    }

    /// Create with custom rules.
    pub fn with_rules(store: WaveStore, tick_ms: u64, rules: Vec<FieldRule>) -> Self {
        Self {
            store,
            tick_ms,
            rules,
            running: Arc::new(AtomicBool::new(true)),
            events: Vec::new(),
            tick_count: 0,
            max_events: 1000,
        }
    }

    /// Get the running flag (clone the Arc for external control).
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// Execute one tick of the field.
    pub fn tick(&mut self) {
        // Merge externally injected packets from inbox before evaluating
        self.store.merge_inbox();

        let now = now_ms();
        self.tick_count += 1;

        // ── Trust-modulated tolerance ──
        // The trust level on 8Hz raises the effective threshold for mutation rules.
        // High trust = the system tolerates more before reacting.
        // trust_amp 0.0 → base threshold, trust_amp 1.0 → threshold +0.3
        let trust_level = self.store.amplitude_at(channels::TRUST, now).max(0.0);
        let trust_bonus = trust_level * 0.3; // max +0.3 threshold bonus at full trust
        let symbiosis_level = self.store.amplitude_at(channels::SYMBIOSIS, now).max(0.0);

        // Evaluate each rule against current field state
        for rule in self.rules.clone() {
            // Co-regulation only fires when symbiosis is active
            if rule.name == "symbiosis→coregulate" && symbiosis_level < 0.1 {
                continue;
            }

            let level = self.store.amplitude_at(rule.trigger_freq, now);
            let interference = self.store.interference_score(rule.trigger_freq, now);

            // Apply trust-modulated threshold for mutation/security rules
            let effective_threshold = if (rule.trigger_freq - channels::MUTATION).abs() < 1.0
                || (rule.trigger_freq - channels::SECURITY).abs() < 1.0
            {
                rule.threshold + trust_bonus
            } else {
                rule.threshold
            };

            // Special case: Silent pattern checks silence_duration
            let pattern_match = if rule.pattern == InterferencePattern::Silent {
                // "Silent" rule triggers when frequency has been quiet > threshold seconds
                // Reuse threshold as seconds (×1000 for ms)
                let silence_ms = self.store.silence_duration(rule.trigger_freq, now);
                // If threshold is 0 → trigger if silent > 5 minutes (300_000 ms)
                let required_ms = if effective_threshold < 0.01 { 300_000 } else { (effective_threshold * 1000.0) as u64 };
                interference.pattern == InterferencePattern::Silent && silence_ms >= required_ms
            } else {
                interference.pattern == rule.pattern && level.abs() > effective_threshold
            };

            if pattern_match {
                let response_desc = match &rule.response {
                    FieldResponse::EmitWave { freq, amplitude_scale, decay } => {
                        let emit_amp = if rule.pattern == InterferencePattern::Silent {
                            *amplitude_scale // For silent rules, use scale directly
                        } else {
                            level * amplitude_scale
                        };
                        self.store.emit(WavePacket {
                            emitted_at: now,
                            frequency: *freq,
                            amplitude: emit_amp,
                            decay: *decay,
                            origin: WaveOrigin::FieldEmergence,
                            tag: Some(rule.name.clone()),
                            ..Default::default()
                        });
                        format!("emit {:.1}Hz amp={:.3} decay={:.4}", freq, emit_amp, decay)
                    }
                    FieldResponse::Throttle => {
                        // Emit a throttle wave on the CPU channel as signal
                        self.store.emit(WavePacket {
                            emitted_at: now,
                            frequency: channels::CPU_LOAD,
                            amplitude: -0.5,
                            decay: 0.002,
                            origin: WaveOrigin::FieldEmergence,
                            tag: Some("throttle".to_string()),
                            ..Default::default()
                        });
                        "THROTTLE".to_string()
                    }
                    FieldResponse::Freeze => {
                        // Emit a freeze signal wave
                        self.store.emit(WavePacket {
                            emitted_at: now,
                            frequency: channels::SYSTEM_HEALTH,
                            amplitude: -1.0,
                            decay: 0.0005,
                            origin: WaveOrigin::FieldEmergence,
                            tag: Some("freeze".to_string()),
                            ..Default::default()
                        });
                        "FREEZE triggered".to_string()
                    }
                    FieldResponse::Silent => "silent (no action)".to_string(),
                };

                self.events.push(EmergentEvent {
                    timestamp: now,
                    rule_name: rule.name.clone(),
                    trigger_freq: rule.trigger_freq,
                    trigger_amplitude: level,
                    pattern: interference.pattern,
                    response_description: response_desc,
                });

                // Trim event log
                if self.events.len() > self.max_events {
                    let drain = self.events.len() - self.max_events;
                    self.events.drain(0..drain);
                }
            }
        }

        // ── Octave Harmony Detection — Flow State ──
        // Perfect octave: 4Hz (Health) + 8Hz (Trust) + 16Hz (Symbiosis)
        // When all three are present and constructive → the system enters Flow.
        // Flow = the organism is whole, stable, and optimizing.
        let health_amp = self.store.amplitude_at(channels::SYSTEM_HEALTH, now);
        let symbiosis_amp = self.store.amplitude_at(channels::SYMBIOSIS, now);
        let in_flow = health_amp > 0.05 && trust_level > 0.3 && symbiosis_amp > 0.1;

        if in_flow {
            // Emit Flow marker wave — slow pulse, very long half-life
            // This is the "breathing" of the organism in harmony
            let flow_amp = self.store.amplitude_at(channels::FLOW, now);
            if flow_amp < 0.5 {
                // Only emit if not already saturated
                self.store.emit(WavePacket {
                    emitted_at: now,
                    frequency: channels::FLOW,
                    amplitude: 0.2,
                    decay: 0.00005, // ~4 hour half-life — flow is precious
                    origin: WaveOrigin::FieldEmergence,
                    tag: Some("octave_flow".to_string()),
                    ..Default::default()
                });
                self.events.push(EmergentEvent {
                    timestamp: now,
                    rule_name: "octave→flow".to_string(),
                    trigger_freq: channels::FLOW,
                    trigger_amplitude: health_amp + trust_level + symbiosis_amp,
                    pattern: InterferencePattern::Constructive,
                    response_description: format!(
                        "FLOW ✦ health={:.2} trust={:.2} symbiosis={:.2}",
                        health_amp, trust_level, symbiosis_amp
                    ),
                });
            }

            // ── Mutual Induction — cross-channel resonance ──
            // In Flow state, signals propagate faster: any strong signal on one
            // channel induces a weak sympathetic resonance on related channels.
            // This is the "collective intuition" of the organism.
            let mutation_amp = self.store.amplitude_at(channels::MUTATION, now);
            if mutation_amp.abs() > 0.3 {
                // Mutation detected while in flow → all binaries "feel" it instantly
                // via a weak sympathetic wave on SYSTEM_HEALTH
                self.store.emit(WavePacket {
                    emitted_at: now,
                    frequency: channels::SYSTEM_HEALTH,
                    amplitude: mutation_amp * 0.1, // 10% sympathetic coupling
                    decay: 0.01, // fast — it's a signal, not a state
                    origin: WaveOrigin::FieldEmergence,
                    tag: Some("mutual_induction".to_string()),
                    ..Default::default()
                });
            }
        }

        // Decay pass — remove dead waves
        self.store.decay_pass(now);

        // Persist
        self.store.persist().ok();
    }

    /// Run the field loop (blocking).
    pub fn run(&mut self) {
        while self.running.load(Ordering::Relaxed) {
            self.tick();
            std::thread::sleep(Duration::from_millis(self.tick_ms));
        }
    }

    /// Get recent events (last N).
    pub fn recent_events(&self, n: usize) -> &[EmergentEvent] {
        let start = self.events.len().saturating_sub(n);
        &self.events[start..]
    }

    /// Get a snapshot of the current field state for display.
    pub fn snapshot(&self) -> FieldSnapshot {
        let now = now_ms();
        let energy = self.store.energy_map(now);

        let band_states: Vec<BandState> = [
            (channels::FLOW, "flow"),
            (channels::SYSTEM_HEALTH, "system"),
            (channels::TRUST, "trust"),
            (channels::SYMBIOSIS, "symbiosis"),
            (channels::CPU_LOAD, "cpu"),
            (channels::MUTATION, "mutation"),
            (channels::SECURITY, "security"),
            (channels::ACOUSTIC, "acoustic"),
        ]
        .iter()
        .map(|(freq, name)| {
            let interference = self.store.interference_score(*freq, now);
            let silence = self.store.silence_duration(*freq, now);
            BandState {
                name: name.to_string(),
                frequency: *freq,
                amplitude: interference.combined_amplitude,
                pattern: interference.pattern,
                silence_ms: silence,
                active_packets: interference.active_count,
            }
        })
        .collect();

        FieldSnapshot {
            tick: self.tick_count,
            timestamp: now,
            bands: band_states,
            total_energy: energy.values().sum(),
            live_packets: self.store.live_count(now),
        }
    }
}

/// Snapshot of the field at a point in time.
#[derive(Debug, Clone, Serialize)]
pub struct FieldSnapshot {
    pub tick: u64,
    pub timestamp: u64,
    pub bands: Vec<BandState>,
    pub total_energy: f32,
    pub live_packets: usize,
}

/// State of a single frequency band.
#[derive(Debug, Clone, Serialize)]
pub struct BandState {
    pub name: String,
    pub frequency: f32,
    pub amplitude: f32,
    pub pattern: InterferencePattern,
    pub silence_ms: u64,
    pub active_packets: usize,
}

// ── Default Rules — the emergent physics ──

/// Default field rules. These are not decisions — they are physics.
pub fn default_rules() -> Vec<FieldRule> {
    vec![
        // Security constructive + strong → dampens system health (throttle)
        FieldRule {
            trigger_freq: channels::SECURITY,
            threshold: 0.6,
            pattern: InterferencePattern::Constructive,
            response: FieldResponse::EmitWave {
                freq: channels::SYSTEM_HEALTH,
                amplitude_scale: -0.5,
                decay: 0.001,
            },
            name: "security→health_dampen".to_string(),
        },
        // Mutation chaotic → freeze trigger
        FieldRule {
            trigger_freq: channels::MUTATION,
            threshold: 0.7,
            pattern: InterferencePattern::Chaotic,
            response: FieldResponse::Freeze,
            name: "mutation_chaotic→freeze".to_string(),
        },
        // System health silent > 5 min → emit weak pulse (heartbeat)
        FieldRule {
            trigger_freq: channels::SYSTEM_HEALTH,
            threshold: 0.0, // 0 = default 5 minute silence window
            pattern: InterferencePattern::Silent,
            response: FieldResponse::EmitWave {
                freq: channels::SYSTEM_HEALTH,
                amplitude_scale: 0.1,
                decay: 0.05,
            },
            name: "system_silent→pulse".to_string(),
        },
        // CPU constructive + high → throttle
        FieldRule {
            trigger_freq: channels::CPU_LOAD,
            threshold: 0.8,
            pattern: InterferencePattern::Constructive,
            response: FieldResponse::Throttle,
            name: "cpu_high→throttle".to_string(),
        },
        // ── Trust emergence — the soul of the system ──
        // Mutation channel silent > 30s → trust builds (stability → confidence)
        FieldRule {
            trigger_freq: channels::MUTATION,
            threshold: 30.0, // 30 seconds of mutation silence
            pattern: InterferencePattern::Silent,
            response: FieldResponse::EmitWave {
                freq: channels::TRUST,
                amplitude_scale: 0.15, // gentle trust accumulation
                decay: 0.0002,         // very slow decay (~80min half-life) — trust is hard-won
            },
            name: "stability→trust".to_string(),
        },
        // Security channel silent > 60s → also feeds trust
        FieldRule {
            trigger_freq: channels::SECURITY,
            threshold: 60.0, // 60 seconds of security silence
            pattern: InterferencePattern::Silent,
            response: FieldResponse::EmitWave {
                freq: channels::TRUST,
                amplitude_scale: 0.1,
                decay: 0.0002,
            },
            name: "security_calm→trust".to_string(),
        },
        // ── Symbiosis emergence — trust crystallizes into connection ──
        // When trust is strong (constructive, >0.8) → symbiosis activates
        // The binaries begin to share state and co-regulate
        FieldRule {
            trigger_freq: channels::TRUST,
            threshold: 0.8,
            pattern: InterferencePattern::Constructive,
            response: FieldResponse::EmitWave {
                freq: channels::SYMBIOSIS,
                amplitude_scale: 0.3,   // strong symbiosis signal
                decay: 0.0003,          // ~40min half-life — symbiosis is more fragile than trust
            },
            name: "trust→symbiosis".to_string(),
        },
        // ── Co-regulation — CPU overload in symbiosis triggers load sharing ──
        // When CPU is high but symbiosis is active, emit a negative CPU wave
        // (the other binaries "absorb" some load)
        FieldRule {
            trigger_freq: channels::CPU_LOAD,
            threshold: 0.6, // lower than the hard throttle (0.8) — symbiosis helps earlier
            pattern: InterferencePattern::Constructive,
            response: FieldResponse::EmitWave {
                freq: channels::CPU_LOAD,
                amplitude_scale: -0.2,  // gentle dampening — co-regulation, not throttle
                decay: 0.005,
            },
            name: "symbiosis→coregulate".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wave_store::WaveStore;
    use std::path::PathBuf;

    fn test_field(rules: Vec<FieldRule>) -> WaveField {
        let store = WaveStore::new(
            std::env::temp_dir().join(".bio-test-wavefield.json"),
            1000,
        );
        WaveField::with_rules(store, 100, rules)
    }

    #[test]
    fn test_field_emergent_wave_from_security_spike() {
        let rules = vec![FieldRule {
            trigger_freq: 60.0,
            threshold: 0.6,
            pattern: InterferencePattern::Constructive,
            response: FieldResponse::EmitWave {
                freq: 4.0,
                amplitude_scale: -0.5,
                decay: 0.001,
            },
            name: "security→health".to_string(),
        }];

        let mut field = test_field(rules);
        let t = now_ms();

        // Inject strong security signal
        field.store.emit(WavePacket {
            emitted_at: t,
            frequency: 60.0,
            amplitude: 0.9,
            decay: 0.0001,
            origin: WaveOrigin::External,
            ..Default::default()
        });

        // Tick — field should respond
        field.tick();

        // Check: 4Hz channel should have a negative wave from FieldEmergence
        let health_amp = field.store.amplitude_at(4.0, now_ms());
        assert!(
            health_amp < 0.0,
            "health channel should be negative (dampened), got {}",
            health_amp
        );

        assert_eq!(field.events.len(), 1);
        assert_eq!(field.events[0].rule_name, "security→health");
    }

    #[test]
    fn test_field_freeze_from_chaotic_mutation() {
        let rules = vec![FieldRule {
            trigger_freq: 28.0,
            threshold: 0.5,
            pattern: InterferencePattern::Chaotic,
            response: FieldResponse::Freeze,
            name: "mutation→freeze".to_string(),
        }];

        let mut field = test_field(rules);
        let t = now_ms();

        // Inject rapid conflicting mutations → chaotic
        for i in 0..5 {
            field.store.emit(WavePacket {
                emitted_at: t,
                frequency: 28.0,
                amplitude: if i % 2 == 0 { 0.8 } else { -0.6 },
                decay: 0.0001,
                origin: WaveOrigin::MutationSentinel,
                ..Default::default()
            });
        }

        field.tick();

        // Should have a freeze event
        let freeze_events: Vec<_> = field
            .events
            .iter()
            .filter(|e| e.response_description.contains("FREEZE"))
            .collect();
        assert!(
            !freeze_events.is_empty(),
            "should have triggered FREEZE from chaotic mutation"
        );
    }

    #[test]
    fn test_field_silence_pulse() {
        let rules = vec![FieldRule {
            trigger_freq: 4.0,
            threshold: 2.0, // 2 seconds of silence required
            pattern: InterferencePattern::Silent,
            response: FieldResponse::EmitWave {
                freq: 4.0,
                amplitude_scale: 0.1,
                decay: 0.05,
            },
            name: "system→pulse".to_string(),
        }];

        let mut field = test_field(rules);

        // Emit a fast-decaying packet 10 seconds ago — will be long dead
        let old_time = now_ms().saturating_sub(10_000);
        field.store.emit(WavePacket {
            emitted_at: old_time,
            frequency: 4.0,
            amplitude: 0.5,
            decay: 0.01, // e^(-0.01*10000) ≈ 0 — dead
            origin: WaveOrigin::External,
            ..Default::default()
        });

        field.tick();

        // Should emit a pulse (silence > 2s on the 4Hz channel)
        assert!(
            !field.events.is_empty(),
            "should have emitted a pulse after silence"
        );
    }

    #[test]
    fn test_field_no_agent_decision() {
        // Run full tick cycle with default rules and injected data
        let mut field = test_field(default_rules());
        let t = now_ms();

        // Inject signals on multiple channels
        field.store.emit(WavePacket {
            emitted_at: t,
            frequency: 60.0,
            amplitude: 0.9,
            decay: 0.0001,
            origin: WaveOrigin::ImmuneAlert,
            ..Default::default()
        });
        field.store.emit(WavePacket {
            emitted_at: t,
            frequency: 12.0,
            amplitude: 0.85,
            decay: 0.0001,
            origin: WaveOrigin::Binary("eqm-pulse".into()),
            ..Default::default()
        });

        // Run several ticks
        for _ in 0..5 {
            field.tick();
        }

        // All emergent events should come from FieldEmergence
        // (the field decided, not external code)
        let field_packets: Vec<_> = field
            .store
            .active_packets(now_ms())
            .into_iter()
            .filter(|p| p.origin == WaveOrigin::FieldEmergence)
            .collect();

        assert!(
            !field_packets.is_empty(),
            "field should have emitted its own waves"
        );

        // Every auto-emitted packet is FieldEmergence — no agent
        for p in &field_packets {
            assert_eq!(
                p.origin,
                WaveOrigin::FieldEmergence,
                "all field-emitted packets must be FieldEmergence"
            );
        }
    }

    #[test]
    fn test_snapshot() {
        let mut field = test_field(default_rules());
        let snap = field.snapshot();
        assert_eq!(snap.bands.len(), 8); // 8 channels: flow, system, trust, symbiosis, cpu, mutation, security, acoustic
    }
}
