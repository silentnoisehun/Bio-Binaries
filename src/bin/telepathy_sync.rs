use bio_binaries::commands::telepathy_sync;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("telepathy-sync");
    let args: Vec<String> = std::env::args().collect();
    match telepathy_sync::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
