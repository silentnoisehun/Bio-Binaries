use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

// Echo-X Protocol Constants
pub const MAGIC: [u8; 2] = [b'E', b'X'];
pub const HEADER_SIZE: usize = 8;
pub const DEFAULT_PORT: u16 = 8888;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Opcode {
    Join = 0x10,
    Task = 0x11,
    Result = 0x12,
    Heartbeat = 0x13,
    Status = 0x14,
    Shutdown = 0x15,
}

impl Opcode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x10 => Some(Self::Join),
            0x11 => Some(Self::Task),
            0x12 => Some(Self::Result),
            0x13 => Some(Self::Heartbeat),
            0x14 => Some(Self::Status),
            0x15 => Some(Self::Shutdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EchoXHeader {
    pub opcode: Opcode,
    pub flags: u8,
    pub payload_len: u16,
}

impl EchoXHeader {
    pub fn new(opcode: Opcode, flags: u8, payload_len: u16) -> Self {
        Self { opcode, flags, payload_len }
    }

    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = MAGIC[0];
        buf[1] = MAGIC[1];
        buf[2] = self.opcode as u8;
        buf[3] = self.flags;
        buf[4] = (self.payload_len >> 8) as u8;
        buf[5] = (self.payload_len & 0xFF) as u8;
        buf[6] = 0; // reserved
        buf[7] = 0; // reserved
        buf
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < HEADER_SIZE {
            return None;
        }
        if buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
            return None;
        }
        let opcode = Opcode::from_byte(buf[2])?;
        let flags = buf[3];
        let payload_len = ((buf[4] as u16) << 8) | (buf[5] as u16);
        Some(Self { opcode, flags, payload_len })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoXMessage {
    pub opcode: Opcode,
    pub flags: u8,
    pub payload: serde_json::Value,
}

impl EchoXMessage {
    pub fn new(opcode: Opcode, payload: serde_json::Value) -> Self {
        Self { opcode, flags: 0, payload }
    }

    pub fn encode(&self) -> Vec<u8> {
        let payload_bytes = serde_json::to_vec(&self.payload).unwrap_or_default();
        let header = EchoXHeader::new(self.opcode, self.flags, payload_bytes.len() as u16);
        let mut buf = header.encode().to_vec();
        buf.extend_from_slice(&payload_bytes);
        buf
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        let header = EchoXHeader::decode(buf)?;
        let payload_start = HEADER_SIZE;
        let payload_end = payload_start + header.payload_len as usize;
        if buf.len() < payload_end {
            return None;
        }
        let payload: serde_json::Value =
            serde_json::from_slice(&buf[payload_start..payload_end]).unwrap_or(serde_json::Value::Null);
        Some(Self {
            opcode: header.opcode,
            flags: header.flags,
            payload,
        })
    }
}

/// Drone client — connects to omega-master
pub struct DroneClient {
    pub name: String,
    pub socket: UdpSocket,
    pub master_addr: SocketAddr,
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
        };

        // Send JOIN
        let join_msg = EchoXMessage::new(Opcode::Join, serde_json::json!({
            "name": name,
            "pid": std::process::id(),
        }));
        client.socket.send_to(&join_msg.encode(), client.master_addr).await?;

        Ok(client)
    }

    pub async fn send_result(&self, result: serde_json::Value) -> std::io::Result<()> {
        let msg = EchoXMessage::new(Opcode::Result, serde_json::json!({
            "name": self.name,
            "result": result,
        }));
        self.socket.send_to(&msg.encode(), self.master_addr).await?;
        Ok(())
    }

    pub async fn send_heartbeat(&self) -> std::io::Result<()> {
        let msg = EchoXMessage::new(Opcode::Heartbeat, serde_json::json!({
            "name": self.name,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));
        self.socket.send_to(&msg.encode(), self.master_addr).await?;
        Ok(())
    }

    pub async fn recv_message(&self) -> std::io::Result<EchoXMessage> {
        let mut buf = [0u8; 65535];
        let (len, _) = self.socket.recv_from(&mut buf).await?;
        EchoXMessage::decode(&buf[..len])
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid Echo-X message"))
    }
}

/// Helper: try to connect as drone if --echo-x flag is provided, run task, send result
pub async fn maybe_echo_x(name: &str, args: &[String], run_fn: impl FnOnce() -> serde_json::Value) -> bool {
    if let Some(pos) = args.iter().position(|a| a == "--echo-x") {
        if let Some(addr) = args.get(pos + 1) {
            match DroneClient::connect(name, addr).await {
                Ok(client) => {
                    let result = run_fn();
                    let _ = client.send_result(result).await;
                    return true;
                }
                Err(e) => {
                    eprintln!("[Echo-X] Connection failed: {}", e);
                }
            }
        }
    }
    false
}
