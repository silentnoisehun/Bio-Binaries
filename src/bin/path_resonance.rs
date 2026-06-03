use bio_binaries::commands::path_resonance;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("path-resonance");
    let args: Vec<String> = std::env::args().collect();
    match path_resonance::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
