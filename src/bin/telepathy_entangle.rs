use bio_binaries::commands::telepathy_entangle;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("telepathy-entangle");
    let args: Vec<String> = std::env::args().collect();
    match telepathy_entangle::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
