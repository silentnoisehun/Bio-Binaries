use bio_binaries::commands::wave_encoder;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("wave-encoder");
    let args: Vec<String> = std::env::args().collect();
    match wave_encoder::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
