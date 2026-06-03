use bio_binaries::commands::brain_synapse;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("brain-synapse");
    let args: Vec<String> = std::env::args().collect();
    match brain_synapse::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
