use crate::{bio_client, output};
use clap::Parser;
use serde::Serialize;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "borg-cube", about = "Parallel command replicator — exponential scaling benchmark")]
pub struct Cli {
    /// Command to replicate
    pub command: String,

    /// Maximum replication power (2^N instances)
    #[arg(long, default_value = "4")]
    pub max_power: u32,


    #[arg(long = "echo-x")]
    pub echo_x: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReplicationStage {
    pub power: u32,
    pub instances: usize,
    pub duration_ms: u128,
    pub throughput: f64,
    pub all_succeeded: bool,
    pub success_count: usize,
    pub fail_count: usize,
}

#[derive(Debug, Serialize)]
pub struct BorgResult {
    pub timestamp: String,
    pub command: String,
    pub max_power: u32,
    pub stages: Vec<ReplicationStage>,
    pub total_instances: usize,
    pub total_duration_ms: u128,
    pub peak_throughput: f64,
    pub assimilation_status: String,
}

pub fn run(command: &str, max_power: u32) -> BorgResult {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let (cmd, cmd_args) = parts.split_first().unwrap_or((&"echo", &[]));

    let mut stages = Vec::new();
    let mut total_instances = 0;
    let total_start = Instant::now();
    let mut peak_throughput = 0.0f64;

    for power in 0..=max_power {
        let n = 2usize.pow(power);
        let start = Instant::now();

        let handles: Vec<_> = (0..n).map(|_| {
            std::process::Command::new(cmd)
                .args(cmd_args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        }).collect();

        let mut success = 0;
        let mut fail = 0;
        for handle in handles {
            match handle {
                Ok(mut child) => {
                    match child.wait() {
                        Ok(status) if status.success() => success += 1,
                        _ => fail += 1,
                    }
                }
                Err(_) => fail += 1,
            }
        }

        let duration = start.elapsed().as_millis();
        let throughput = if duration > 0 { n as f64 / (duration as f64 / 1000.0) } else { n as f64 };
        if throughput > peak_throughput { peak_throughput = throughput; }
        total_instances += n;

        stages.push(ReplicationStage {
            power,
            instances: n,
            duration_ms: duration,
            throughput: (throughput * 10.0).round() / 10.0,
            all_succeeded: fail == 0,
            success_count: success,
            fail_count: fail,
        });
    }

    let total_duration = total_start.elapsed().as_millis();
    let status = if stages.iter().all(|s| s.all_succeeded) { "FULLY_ASSIMILATED" }
        else if stages.iter().any(|s| s.all_succeeded) { "PARTIAL_ASSIMILATION" }
        else { "RESISTANCE_DETECTED" };

    BorgResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        command: command.to_string(),
        max_power,
        stages,
        total_instances,
        total_duration_ms: total_duration,
        peak_throughput: (peak_throughput * 10.0).round() / 10.0,
        assimilation_status: status.to_string(),
    }
}

fn print_pretty(result: &BorgResult) {
    output::banner("BORG-CUBE", "Machine-Brain / Parallel Replicator", "🤖");

    output::section("Configuration");
    output::kv("Command", &result.command);
    output::kv("Max Power", &result.max_power.to_string());

    println!();
    output::section("Replication Stages");
    for stage in &result.stages {
        let status = if stage.all_succeeded { "OK" } else { "FAIL" };
        output::kv(
            &format!("2^{} = {} instances", stage.power, stage.instances),
            &format!("{}ms  {:.1}/s  {} ({}/{})",
                stage.duration_ms, stage.throughput, status, stage.success_count, stage.instances),
        );
    }

    println!();
    output::kv("Total Instances", &result.total_instances.to_string());
    output::kv("Total Duration", &format!("{}ms", result.total_duration_ms));
    output::kv("Peak Throughput", &format!("{:.1}/s", result.peak_throughput));
    output::summary("borg-cube", &result.assimilation_status);
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.command, cli.max_power);

    if let Some(addr) = &cli.echo_x {
        if let Ok(client) = bio_client::DroneClient::connect("borg-cube", addr).await {
            let max_power_str = result.max_power.to_string();
            let instances_str = result.total_instances.to_string();
            let _ = client.send_result(&[
                ("status", b"OK"),
                ("max_power", max_power_str.as_bytes()),
                ("total_instances", instances_str.as_bytes()),
                ("assimilation_status", result.assimilation_status.as_bytes()),
            ]).await;
        }
    }

    print_pretty(&result);
    Ok(String::new())
}
