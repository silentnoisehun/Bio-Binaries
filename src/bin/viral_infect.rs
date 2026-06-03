use bio_binaries::commands::viral_infect;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("viral-infect");
    let args: Vec<String> = std::env::args().collect();
    match viral_infect::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
