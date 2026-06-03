use bio_binaries::commands::plasmid_dream;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("plasmid-dream");
    let args: Vec<String> = std::env::args().collect();
    match plasmid_dream::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
