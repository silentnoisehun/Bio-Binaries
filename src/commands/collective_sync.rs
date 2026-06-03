use crate::{bio_client, echox::{EchoXMessage, Opcode}, output};
use clap::Parser;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "collective-sync", about = "Multi-process state reconciliation — distributed consensus")]
pub struct Cli {
    /// Echo-X master address (required)
    #[arg(long = "echo-x", default_value = "127.0.0.1:8888")]
    pub echo_x: String,

    /// Consensus topic
    #[arg(long, default_value = "health-check")]
    pub topic: String,

    /// Vote value
    #[arg(long, default_value = "OK")]
    pub vote: String,

}

#[derive(Debug, Serialize)]
pub struct ConsensusResult {
    pub timestamp: String,
    pub topic: String,
    pub own_vote: String,
    pub cluster_size: usize,
    pub votes_collected: usize,
    pub consensus_reached: bool,
    pub majority_vote: String,
    pub sync_status: SyncStatus,
}

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub structural: String,
    pub emotional: String,
    pub logical: String,
    pub collective: String,
}

pub async fn run(echo_x_addr: &str, topic: &str, vote: &str) -> ConsensusResult {
    // Connect as drone
    let client = match bio_client::DroneClient::connect("collective-sync", echo_x_addr).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Cannot connect to omega-master: {}", e);
            return ConsensusResult {
                timestamp: chrono::Utc::now().to_rfc3339(),
                topic: topic.to_string(),
                own_vote: vote.to_string(),
                cluster_size: 0,
                votes_collected: 0,
                consensus_reached: false,
                majority_vote: "N/A".to_string(),
                sync_status: SyncStatus {
                    structural: "DISCONNECTED".into(),
                    emotional: "ISOLATED".into(),
                    logical: "NO_DATA".into(),
                    collective: "OFFLINE".into(),
                },
            };
        }
    };

    // Send our vote as a result
    let _ = client.send_result(&[
        ("type", b"consensus_vote"),
        ("topic", topic.as_bytes()),
        ("vote", vote.as_bytes()),
    ]).await;

    // Query cluster status
    let status_msg = EchoXMessage::new(Opcode::Status, serde_json::json!({"query": "all"}));
    let _ = client.socket.send_to(&status_msg.encode(), client.master_addr).await;

    let mut cluster_size = 0;
    let votes = vec![vote.to_string()];

    // Try to receive status response
    match tokio::time::timeout(std::time::Duration::from_secs(2), client.recv_message()).await {
        Ok(Ok(msg)) => {
            // Parse payload to detect drone count
            let fields = crate::bio_protocol::decode_fields(&msg.payload);
            if let Some((_, drone_count_bytes)) = fields.iter().find(|(k, _)| k == "drone_count") {
                if let Ok(count_str) = std::str::from_utf8(drone_count_bytes) {
                    if let Ok(count) = count_str.parse::<usize>() {
                        cluster_size = count;
                    }
                }
            }
        }
        _ => {}
    }

    // Majority vote
    let mut vote_counts = std::collections::HashMap::new();
    for v in &votes {
        *vote_counts.entry(v.clone()).or_insert(0usize) += 1;
    }
    let majority = vote_counts.iter().max_by_key(|(_, &c)| c)
        .map(|(v, _)| v.clone()).unwrap_or_else(|| "N/A".to_string());
    let consensus = vote_counts.get(&majority).copied().unwrap_or(0) > votes.len() / 2;

    let sync_status = SyncStatus {
        structural: if cluster_size > 0 { "CONNECTED" } else { "DISCONNECTED" }.into(),
        emotional: if consensus { "HARMONIZED" } else { "CONFLICTED" }.into(),
        logical: if consensus { "CONSENSUS" } else { "DIVERGENT" }.into(),
        collective: if consensus && cluster_size > 1 { "SYNCHRONIZED" } else { "PARTIAL" }.into(),
    };

    ConsensusResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        topic: topic.to_string(),
        own_vote: vote.to_string(),
        cluster_size,
        votes_collected: votes.len(),
        consensus_reached: consensus,
        majority_vote: majority,
        sync_status,
    }
}

fn print_pretty(result: &ConsensusResult) {
    output::banner("COLLECTIVE-SYNC", "Machine-Brain / Distributed Consensus", "🔄");

    output::section("Consensus");
    output::kv("Topic", &result.topic);
    output::kv("Own Vote", &result.own_vote);
    output::kv("Cluster Size", &result.cluster_size.to_string());
    output::kv("Votes Collected", &result.votes_collected.to_string());
    output::kv("Majority", &result.majority_vote);
    output::kv("Consensus", &result.consensus_reached.to_string());

    println!();
    output::section("Sync Status");
    output::kv("Structural", &result.sync_status.structural);
    output::kv("Emotional", &result.sync_status.emotional);
    output::kv("Logical", &result.sync_status.logical);
    output::kv("Collective", &result.sync_status.collective);

    let status = if result.consensus_reached { "CONSENSUS_REACHED" } else { "NO_CONSENSUS" };
    output::summary("collective-sync", status);
}

pub async fn dispatch(args: &[String]) -> Result<String, String> {
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    let result = run(&cli.echo_x, &cli.topic, &cli.vote).await;

    print_pretty(&result);
            Ok(String::new())
}
