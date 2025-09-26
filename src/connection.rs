use anyhow::Result;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;

pub type RelayConnection = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct RelayManager {
    relays: HashMap<String, RelayStatus>,
}

#[derive(Debug, Clone)]
pub enum RelayStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
}


impl RelayManager {
    pub fn new() -> Self {
        Self {
            relays: HashMap::new(),
        }
    }

    pub async fn add_relay(&mut self, url: &str) -> Result<()> {
        let relay_url = Url::parse(url)?;

        if relay_url.scheme() != "ws" && relay_url.scheme() != "wss" {
            return Err(anyhow::anyhow!("Invalid relay URL scheme: {}", relay_url.scheme()));
        }

        self.relays.insert(url.to_string(), RelayStatus::Disconnected);
        Ok(())
    }

    pub async fn connect_relay(&mut self, url: &str) -> Result<RelayConnection> {
        self.relays.insert(url.to_string(), RelayStatus::Connecting);

        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                self.relays.insert(url.to_string(), RelayStatus::Connected);
                println!("Connected to relay: {}", url);
                Ok(ws_stream)
            }
            Err(e) => {
                self.relays.insert(url.to_string(), RelayStatus::Failed);
                Err(anyhow::anyhow!("Failed to connect to relay {}: {}", url, e))
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_relay_status(&self, url: &str) -> Option<&RelayStatus> {
        self.relays.get(url)
    }

    #[allow(dead_code)]
    pub fn connected_relays(&self) -> Vec<&String> {
        self.relays
            .iter()
            .filter_map(|(url, status)| {
                matches!(status, RelayStatus::Connected).then_some(url)
            })
            .collect()
    }
}