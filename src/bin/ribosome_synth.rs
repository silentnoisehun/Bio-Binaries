use bio_binaries::commands::ribosome_synth;

#[tokio::main]
async fn main() {
    bio_binaries::bio_auth_gate!("ribosome-synth");
    let args: Vec<String> = std::env::args().collect();
    match ribosome_synth::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
