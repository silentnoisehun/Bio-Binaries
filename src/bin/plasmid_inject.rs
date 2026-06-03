use bio_binaries::commands::plasmid_inject;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("plasmid-inject");
    let args: Vec<String> = std::env::args().collect();
    match plasmid_inject::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
