use bio_binaries::commands::aether_fabric;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("aether-fabric");
    let args: Vec<String> = std::env::args().collect();
    match aether_fabric::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
