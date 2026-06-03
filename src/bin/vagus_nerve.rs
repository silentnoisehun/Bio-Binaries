use bio_binaries::commands::vagus_nerve;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("vagus-nerve");
    let args: Vec<String> = std::env::args().collect();
    match vagus_nerve::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
