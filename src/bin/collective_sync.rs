use bio_binaries::commands::collective_sync;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("collective-sync");
    let args: Vec<String> = std::env::args().collect();
    match collective_sync::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
