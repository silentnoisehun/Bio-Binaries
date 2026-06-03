use bio_binaries::commands::wave_cryo_tx;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("wave-cryo-tx");
    let args: Vec<String> = std::env::args().collect();
    match wave_cryo_tx::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
