use bio_binaries::commands::borg_cube;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("borg-cube");
    let args: Vec<String> = std::env::args().collect();
    match borg_cube::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
