use bio_binaries::commands::wave_field_bin;

#[tokio::main]
async fn main() {
//    bio_binaries::bio_auth_gate!("wave-field");
    let args: Vec<String> = std::env::args().collect();
    match wave_field_bin::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
