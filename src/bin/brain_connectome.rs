use bio_binaries::commands::brain_connectome;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("brain-connectome");
    let args: Vec<String> = std::env::args().collect();
    match brain_connectome::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
