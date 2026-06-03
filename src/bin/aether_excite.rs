use bio_binaries::commands::aether_excite;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("aether-excite");
    let args: Vec<String> = std::env::args().collect();
    match aether_excite::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
