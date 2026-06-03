use bio_binaries::commands::magneto_acoustic;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("magneto-acoustic");
    let args: Vec<String> = std::env::args().collect();
    match magneto_acoustic::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
