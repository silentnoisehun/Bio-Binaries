use bio_binaries::commands::mutation_sentinel;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("mutation-sentinel");
    let args: Vec<String> = std::env::args().collect();
    match mutation_sentinel::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
