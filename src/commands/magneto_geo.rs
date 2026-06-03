use crate::{bio_client, magneto};
use crate::magneto::GeoResult;
use clap::Parser;

#[derive(Parser)]
#[command(name = "magneto-geo", about = "Error hotspot detector — code quality heatmap scanner")]
pub struct Cli {
    /// Project directory to scan
    #[arg(default_value = ".")]
    pub path: String,

    /// Maximum depth
    #[arg(long, default_value = "10")]
    pub depth: usize,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

pub fn run(path: &str, depth: usize) -> GeoResult {
    magneto::run(path, depth)
}

fn format_pretty(result: &GeoResult) -> String {
    let mut out = String::new();

    // Banner
    out.push_str(&format!("{}\n", "╔══════════════════════════════════════════╗"));
    out.push_str(&format!("║  {} {} \n", "🧲", "MAGNETO-GEO"));
    out.push_str(&format!("║  Layer: {}\n", "Resonance / Error Hotspot Detector"));
    out.push_str(&format!("{}\n\n", "╚══════════════════════════════════════════╝"));

    // Scan Summary
    out.push_str(&format!("  ▸ Scan Summary\n"));
    out.push_str(&format!("    Root: {}\n", result.root));
    out.push_str(&format!("    Files Scanned: {}\n", result.files_scanned));
    out.push_str(&format!("    Total Hotspots: {}\n", result.total_hotspots));
    out.push_str(&format!("    Tension Score: {:.2}\n", result.tension_score));

    out.push('\n');
    out.push_str(&format!("  ▸ Severity Distribution\n"));
    for (sev, count) in &result.severity_counts {
        out.push_str(&format!("    {}: {}\n", sev, count));
    }

    out.push('\n');
    out.push_str(&format!("  ▸ Top Hotspots (strongest magnetic charge)\n"));
    for h in result.hotspots.iter().take(20) {
        let charge_str = format!("{:.1}", h.magnetic_charge);
        out.push_str(&format!(
            "    [{}] {}:{}: {} | {}\n",
            h.pattern, h.file, h.line, charge_str, &h.text[..h.text.len().min(80)]
        ));
    }

    out.push_str(&format!("\n  ⟫ magneto-geo :: {} hotspots, tension={:.2}\n\n", result.total_hotspots, result.tension_score));
    out
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.path, cli.depth);

    // Echo-X support
    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("magneto-geo", addr).await {
            let result_str = serde_json::to_string(&result).unwrap_or_default();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("data", result_str.as_bytes()),
            ]).await;
        }
    }

    Ok(format_pretty(&result))
}
