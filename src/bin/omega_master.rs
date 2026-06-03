use bio_binaries::commands::omega_master;

#[tokio::main]
async fn main() {
//    bio_binaries::bio_auth_gate!("omega-master");
    let args: Vec<String> = std::env::args().collect();
    match omega_master::dispatch(&args).await {
        Ok(output) => print!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
