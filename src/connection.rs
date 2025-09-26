use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
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
    Failed(String),
}

// Nostr protocol message types (client to relay)
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "EVENT")]
    Event(Value),
    #[serde(rename = "REQ")]
    Request {
        subscription_id: String,
        filters: Vec<Value>,
    },
    #[serde(rename = "CLOSE")]
    Close { subscription_id: String },
}

// Nostr protocol message types (relay to client)
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RelayMessage {
    Event(String, String, Value),      // ["EVENT", subscription_id, event]
    Ok(String, String, bool, String),  // ["OK", event_id, accepted, message]
    Eose(String, String),              // ["EOSE", subscription_id]
    Closed(String, String, String),    // ["CLOSED", subscription_id, message]
    Notice(String, String),            // ["NOTICE", message]
}

impl RelayManager {
    pub fn new() -> Self {
        Self {
            relays: HashMap::new(),
        }
    }

    pub async fn add_relay(&mut self, url: &str) -> Result<()> {
        let relay_url = Url::parse(url)?;

        // Validate WebSocket URL
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
                self.relays.insert(url.to_string(), RelayStatus::Failed(e.to_string()));
                Err(anyhow::anyhow!("Failed to connect to relay {}: {}", url, e))
            }
        }
    }

    pub fn get_relay_status(&self, url: &str) -> Option<&RelayStatus> {
        self.relays.get(url)
    }

    pub fn connected_relays(&self) -> Vec<&String> {
        self.relays
            .iter()
            .filter_map(|(url, status)| {
                matches!(status, RelayStatus::Connected).then_some(url)
            })
            .collect()
    }
}

// Helper functions for creating Nostr protocol messages
impl ClientMessage {
    pub fn request(subscription_id: String, filters: Vec<Value>) -> Self {
        ClientMessage::Request {
            subscription_id,
            filters,
        }
    }

    pub fn close(subscription_id: String) -> Self {
        ClientMessage::Close { subscription_id }
    }

    pub fn to_json_message(&self) -> Result<Message> {
        let json_str = serde_json::to_string(self)?;
        Ok(Message::Text(json_str.into()))
    }
}