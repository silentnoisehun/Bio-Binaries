use bio_binaries::commands::hox_diff;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("hox-diff");
    let args: Vec<String> = std::env::args().collect();
    match hox_diff::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
