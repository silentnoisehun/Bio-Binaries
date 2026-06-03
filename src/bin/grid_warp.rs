use bio_binaries::commands::grid_warp;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("grid-warp");
    let args: Vec<String> = std::env::args().collect();
    match grid_warp::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
