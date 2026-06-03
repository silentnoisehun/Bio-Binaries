use bio_binaries::commands::mycelium_spread;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("mycelium-spread");
    let args: Vec<String> = std::env::args().collect();
    match mycelium_spread::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
