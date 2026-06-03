use bio_binaries::commands::eqm_pulse;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("eqm-pulse");
    let args: Vec<String> = std::env::args().collect();
    match eqm_pulse::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
