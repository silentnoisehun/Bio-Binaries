use bio_binaries::commands::nexus_logic;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("nexus-logic");
    let args: Vec<String> = std::env::args().collect();
    match nexus_logic::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
