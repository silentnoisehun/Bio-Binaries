use bio_binaries::commands::eqm_methy;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("eqm-methy");
    let args: Vec<String> = std::env::args().collect();
    match eqm_methy::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
