/// Homeostasis — System-wide balance and self-healing.
///
/// The Homeostasis module monitors the WaveField and system telemetry
/// to dynamically adjust Digital Leash parameters via CRISPR patches.

use crate::wave_store::{WaveStore, channels, now_ms};
use crate::leash::{DigitalLeash, AlertType, AlertSeverity};
use crate::bio_protocol::{BioOp, BioMessage};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct HomeostasisAgent {
    pub store: WaveStore,
    pub leash: Arc<Mutex<DigitalLeash>>,
    pub drones: Option<crate::commands::omega_master::DroneMap>,
    pub socket: Option<Arc<tokio::net::UdpSocket>>,
    pub queen_key: Option<crate::commands::omega_master::SharedKey>,
    pub tick_ms: u64,
    pub llm_client: reqwest::Client,
}

impl HomeostasisAgent {
    pub fn new(
        store: WaveStore, 
        leash: Arc<Mutex<DigitalLeash>>,
        drones: Option<crate::commands::omega_master::DroneMap>,
        socket: Option<Arc<tokio::net::UdpSocket>>,
        queen_key: Option<crate::commands::omega_master::SharedKey>,
    ) -> Self {
        Self {
            store,
            leash,
            drones,
            socket,
            queen_key,
            tick_ms: 2000, // Slower tick for LLM
            llm_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Run the homeostasis loop.
    pub async fn run(mut self) {
        eprintln!("[HOMEO] Starting homeostasis control loop ({}ms tick)", self.tick_ms);
        
        loop {
            self.evaluate().await;
            tokio::time::sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }

    async fn broadcast_patch(&self, key: &str, value: &[u8]) {
        if let (Some(drones), Some(socket), Some(queen_key)) = (&self.drones, &self.socket, &self.queen_key) {
            let payload = crate::bio_protocol::encode_fields(&[(key, value)]);
            let drones_read = drones.read().await;
            for drone in drones_read.values() {
                if drone.protocol == "v2-bio" {
                    if let Ok(addr) = drone.addr.parse::<std::net::SocketAddr>() {
                        let mut msg = BioMessage::new(BioOp::CrisprPatch, 0, payload.clone());
                        msg.sign(queen_key.as_bytes());
                        let packet = msg.encode();
                        let _ = socket.send_to(&packet, addr).await;
                    }
                }
            }
        }
    }

    async fn broadcast_alert(&self, alert_type: AlertType, severity: AlertSeverity) {
        if let (Some(drones), Some(socket), Some(queen_key)) = (&self.drones, &self.socket, &self.queen_key) {
            let mut payload = vec![alert_type as u8, severity as u8];
            payload.extend_from_slice(b"HOMEOSTASIS_AUTO_CORRECTION");
            
            let drones_read = drones.read().await;
            for drone in drones_read.values() {
                if drone.protocol == "v2-bio" {
                    if let Ok(addr) = drone.addr.parse::<std::net::SocketAddr>() {
                        let mut msg = BioMessage::new(BioOp::ImmuneAlert, 0, payload.clone());
                        msg.sign(queen_key.as_bytes());
                        let packet = msg.encode();
                        let _ = socket.send_to(&packet, addr).await;
                    }
                }
            }
        }
    }

    /// Asks the Local LLM (Ítélőszék) for a judgement based on WaveField energy.
    async fn ask_iteloszek(&self, energy_map: &std::collections::HashMap<u32, f32>) -> Option<serde_json::Value> {
        let system_prompt = "Te, az Ítélőszék (Judgement Seat), egy biológiai szoftver-organizmus immunrendszerét irányítod. \
        Elemezd a betáplált hullámteret (WaveField). Döntsd el a védekező lépéseket. \
        Válaszként KIZÁRÓLAG egy valid JSON-t küldj vissza! \
        Lehetséges JSON válaszok: \
        1. { \"action\": \"THROTTLE\", \"target\": \"CPU\", \"value\": 20, \"reason\": \"<ok>\" } (Ha veszélyt látsz 60Hz-en vagy túl nagy a terhelés) \
        2. { \"action\": \"RELAX\", \"target\": \"CPU\", \"value\": 100, \"reason\": \"<ok>\" } (Ha a mező békés) \
        3. { \"action\": \"NONE\", \"reason\": \"Minden stabil\" }";

        let fever_amp = energy_map.get(&(channels::FEVER as u32)).copied().unwrap_or(0.0);
        let user_prompt = format!(
            "Jelenlegi WaveField Energy Map (Frekvencia -> Amplitúdó): {:?}\n\
            CPU Hőmérséklet (FEVER csatorna {:.0}Hz): amplitúdó={:.2} (>0.5 = MELEG, >1.0 = LÁZ!)",
            energy_map, channels::FEVER, fever_amp
        );

        let payload = serde_json::json!({
            "model": "gemini-3-flash-preview:latest",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.1,
            "response_format": { "type": "json_object" }
        });

        // Use local Ollama API explicitly
        let res = self.llm_client.post("http://127.0.0.1:11434/v1/chat/completions")
            .json(&payload)
            .send()
            .await
            .ok()?;

        let json_body: serde_json::Value = res.json().await.ok()?;
        
        // Extract the content string which should be JSON
        let content_str = json_body["choices"][0]["message"]["content"].as_str()?;
        serde_json::from_str(content_str).ok()
    }

    /// Evaluate system balance and apply corrections using the LLM.
    pub async fn evaluate(&mut self) {
        // Refresh WaveStore from disk
        if let Ok(store) = WaveStore::load(&crate::wave_store::default_path(), 10000) {
            self.store = store;
        }

        let now = now_ms();
        let energy_map = self.store.energy_map(now);
        let total_energy: f32 = energy_map.values().sum();

        if total_energy < 0.1 {
            // Calm field, no need to wake the Ítélőszék, but slowly relax limits
            let mut leash = self.leash.lock().await;
            if leash.metabolic.limits.max_cpu_percent < 100.0 {
                leash.metabolic.limits.max_cpu_percent += 1.0;
            }
            return;
        }

        eprintln!("[HOMEO] Field active (Total Energy: {:.2}). Consulting Ítélőszék...", total_energy);
        
        if let Some(judgement) = self.ask_iteloszek(&energy_map).await {
            eprintln!("[HOMEO] Ítélőszék Döntött: {}", judgement.to_string());
            
            let action = judgement["action"].as_str().unwrap_or("NONE");
            let reason = judgement["reason"].as_str().unwrap_or("Ismeretlen indok");

            match action {
                "THROTTLE" => {
                    let mut leash = self.leash.lock().await;
                    let target = judgement["target"].as_str().unwrap_or("CPU");
                    if target == "CPU" {
                        let val = judgement["value"].as_f64().unwrap_or(20.0) as f32;
                        leash.metabolic.limits.max_cpu_percent = val;
                        leash.metabolic.limits.cooldown_secs = 120;
                        
                        eprintln!("[HOMEO] CRITICAL: Ítélőszék szerint throttling szükséges. Ok: {}. Új CPU limit: {}%", reason, val);
                        
                        self.broadcast_alert(AlertType::IntrusionDetected, AlertSeverity::High).await;
                        let cpu_bytes = leash.metabolic.limits.max_cpu_percent.to_le_bytes();
                        self.broadcast_patch("max_cpu", &cpu_bytes).await;
                    }
                },
                "RELAX" => {
                    let mut leash = self.leash.lock().await;
                    let val = judgement["value"].as_f64().unwrap_or(100.0) as f32;
                    leash.metabolic.limits.max_cpu_percent = val;
                    leash.metabolic.limits.cooldown_secs = 0;
                    
                    eprintln!("[HOMEO] Ítélőszék feloldja a korlátozásokat. Ok: {}. Új CPU limit: {}%", reason, val);
                    
                    let cpu_bytes = leash.metabolic.limits.max_cpu_percent.to_le_bytes();
                    self.broadcast_patch("max_cpu", &cpu_bytes).await;
                },
                "NONE" | _ => {
                    eprintln!("[HOMEO] Ítélőszék jóváhagyta az állapotot: {}", reason);
                }
            }
        } else {
            eprintln!("[HOMEO] Ítélőszék is silent (No AI response). Falling back to basic instincts.");
            // Fallback baseline homeostasis
            let sec_interference = self.store.interference_score(channels::SECURITY, now);
            if sec_interference.combined_amplitude > 0.7 {
                eprintln!("[HOMEO-FALLBACK] CRITICAL: Security anomaly ({:.2}). Throttling drones.", sec_interference.combined_amplitude);
                let mut leash = self.leash.lock().await;
                leash.metabolic.limits.max_cpu_percent = 20.0;
                self.broadcast_alert(AlertType::IntrusionDetected, AlertSeverity::High).await;
            }
        }
    }
}

/// Dispatch function for Homeostasis commands (CLI).
pub async fn dispatch(args: &[String]) -> Result<String, String> {
    use clap::{Parser, Subcommand};

    #[derive(Parser)]
    #[command(name = "homeostasis", about = "Homeostasis Control System", no_binary_name = true)]
    struct Cli {
        #[command(subcommand)]
        command: Commands,
    }

    #[derive(Subcommand)]
    enum Commands {
        /// Monitor the field and show current balance status
        Status,
        /// Start the homeostasis loop (Queen mode)
        Run {
            #[arg(long, default_value = "1000")]
            tick_ms: u64,
        },
    }

    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    match cli.command {
        Commands::Status => {
            let store = crate::wave_store::WaveStore::new(crate::wave_store::default_path(), 10000);
            let now = now_ms();
            let energy = store.energy_map(now);
            
            println!("--- HOMEOSTASIS STATUS ---");
            println!("Field Energy: {:.3}", energy.values().sum::<f32>());
            for (freq, e) in energy {
                println!("  Channel {:.0}Hz: energy={:.3}", freq, e);
            }
            Ok(String::new())
        }
        Commands::Run { tick_ms: _ } => {
            let store = crate::wave_store::WaveStore::new(crate::wave_store::default_path(), 10000);
            let leash = Arc::new(Mutex::new(DigitalLeash::new("homeo-queen", 0)));
            let agent = HomeostasisAgent::new(store, leash, None, None, None);
            agent.run().await;
            Ok(String::new())
        }
    }
}
