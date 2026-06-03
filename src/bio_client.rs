use crate::bio_protocol::{BioMessage, BioOp};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// BioMessage-based drone client — replaces EchoX
pub struct DroneClient {
    pub name: String,
    pub socket: UdpSocket,
    pub master_addr: SocketAddr,
    pub generation: u32,
}

impl DroneClient {
    pub async fn connect(name: &str, master_addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let master_addr: SocketAddr = master_addr.parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let client = Self {
            name: name.to_string(),
            socket,
            master_addr,
            generation: 0,
        };

        // Send v2 BioMessage JOIN
        let payload = crate::bio_protocol::encode_fields(&[
            ("name", name.as_bytes()),
            ("pid", std::process::id().to_string().as_bytes()),
        ]);
        let join_msg = BioMessage::new(BioOp::Join, 0, payload);
        // Note: JOIN messages don't need signing (unauthenticated)
        client.socket.send_to(&join_msg.encode(), client.master_addr).await?;

        Ok(client)
    }

    pub async fn send_result(&self, payload_fields: &[(&str, &[u8])]) -> std::io::Result<()> {
        let payload = crate::bio_protocol::encode_fields(payload_fields);
        let msg = BioMessage::new(BioOp::Result, self.generation, payload);
        // Drones don't have the QueenKey, so they can't sign
        // Omega-master should trust internal drones
        self.socket.send_to(&msg.encode(), self.master_addr).await?;
        Ok(())
    }

    pub async fn send_heartbeat(&self) -> std::io::Result<()> {
        let payload = crate::bio_protocol::encode_fields(&[
            ("name", self.name.as_bytes()),
            ("timestamp", chrono::Utc::now().to_rfc3339().as_bytes()),
        ]);
        let msg = BioMessage::new(BioOp::Heartbeat, self.generation, payload);
        self.socket.send_to(&msg.encode(), self.master_addr).await?;
        Ok(())
    }

    pub async fn recv_message(&self) -> std::io::Result<BioMessage> {
        let mut buf = [0u8; 65535];
        let (len, _) = self.socket.recv_from(&mut buf).await?;
        BioMessage::decode(&buf[..len])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}
