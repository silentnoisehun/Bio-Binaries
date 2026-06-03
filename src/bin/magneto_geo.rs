use bio_binaries::commands::magneto_geo;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("magneto-geo");
    let args: Vec<String> = std::env::args().collect();
    match magneto_geo::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
