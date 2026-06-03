use bio_binaries::commands::iron_resonate;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("iron-resonate");
    let args: Vec<String> = std::env::args().collect();
    match iron_resonate::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
