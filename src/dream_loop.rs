//! DreamLoop — Stub verzió
//! Teljes integráció később az ora/microscope-memory-vel

use std::time::Duration;

pub async fn run_dream_loop() {
    eprintln!("[DREAM] 💤 Dream loop stub — full implementation pending");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
