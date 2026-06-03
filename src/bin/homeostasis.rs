use bio_binaries::commands::homeostasis;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match homeostasis::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}