use bio_binaries::commands::microscope_mem;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("microscope-mem");
    let args: Vec<String> = std::env::args().collect();
    match microscope_mem::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}