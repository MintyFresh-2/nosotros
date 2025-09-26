use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{self, Value};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::connection::RelayManager;
use crate::nostr::{keypair_from_hex, NostrEvent};

pub struct PostCommand {
    pub message_content: String,
    pub relay_url: String,
    pub author_private_key_hex: String,
}

impl PostCommand {
    pub fn new(text: String, relay_url: String, private_key_hex: String) -> Self {
        Self {
            message_content: text,
            relay_url,
            author_private_key_hex: private_key_hex,
        }
    }

    pub async fn execute(&self) -> Result<String> {
        println!("Creating and posting event: {}", self.message_content);

        let author_keypair = keypair_from_hex(&self.author_private_key_hex)
            .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))?;

        let text_note_event = NostrEvent::new_text_note(self.message_content.clone(), &author_keypair)
            .map_err(|e| anyhow::anyhow!("Failed to create event: {}", e))?;

        println!("Created event with ID: {}", text_note_event.id);
        println!("Public key: {}", text_note_event.pubkey);

        let mut relay_manager = RelayManager::new();
        relay_manager.add_relay(&self.relay_url).await
            .map_err(|e| anyhow::anyhow!("Invalid relay URL: {}", e))?;

        let mut relay_connection = relay_manager.connect_relay(&self.relay_url).await
            .map_err(|e| anyhow::anyhow!("Failed to connect to relay: {}", e))?;

        println!("Connected to relay: {}", self.relay_url);

        let event_json = text_note_event.to_json_value()?;
        let event_message = serde_json::json!(["EVENT", event_json]);
        let event_message_text = serde_json::to_string(&event_message)?;

        relay_connection.send(Message::Text(event_message_text.into())).await
            .map_err(|e| anyhow::anyhow!("Failed to send event to relay: {}", e))?;

        println!("📤 Event sent, waiting for relay response...");

        let relay_response = timeout(Duration::from_secs(10), relay_connection.next()).await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for relay response"))?;

        match relay_response {
            Some(Ok(Message::Text(response_text))) => {
                let response_json: Value = serde_json::from_str(&response_text.to_string())
                    .map_err(|e| anyhow::anyhow!("Failed to parse relay response: {}", e))?;

                self.handle_relay_response(&response_json, &text_note_event.id)?;

                println!("✅ Event published successfully!");
                println!("Event ID: {}", text_note_event.id);
                Ok(text_note_event.id)
            }
            Some(Ok(Message::Close(_))) => {
                Err(anyhow::anyhow!("Relay closed connection before responding"))
            }
            Some(Ok(Message::Binary(_))) => {
                Err(anyhow::anyhow!("Received unexpected binary message from relay"))
            }
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                Err(anyhow::anyhow!("Received ping/pong instead of response"))
            }
            Some(Ok(Message::Frame(_))) => {
                Err(anyhow::anyhow!("Received unexpected frame message"))
            }
            Some(Err(e)) => {
                Err(anyhow::anyhow!("WebSocket error while waiting for response: {}", e))
            }
            None => {
                Err(anyhow::anyhow!("No response received from relay"))
            }
        }
    }

    fn handle_relay_response(&self, response: &Value, expected_event_id: &str) -> Result<()> {
        if let Some(response_array) = response.as_array() {
            if response_array.is_empty() {
                return Err(anyhow::anyhow!("Empty response from relay"));
            }

            match response_array[0].as_str() {
                Some("OK") => {
                    if response_array.len() < 4 {
                        return Err(anyhow::anyhow!("Invalid OK response format"));
                    }

                    let event_id = response_array[1].as_str().unwrap_or("");
                    let accepted = response_array[2].as_bool().unwrap_or(false);
                    let message = response_array[3].as_str().unwrap_or("");

                    // Verify the event ID matches what we sent
                    if event_id != expected_event_id {
                        return Err(anyhow::anyhow!(
                            "Event ID mismatch: expected {}, got {}",
                            expected_event_id,
                            event_id
                        ));
                    }

                    if accepted {
                        println!("📨 Relay accepted event: {}", message);
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Relay rejected event: {}", message))
                    }
                }
                Some("NOTICE") => {
                    let notice_message = response_array.get(1)
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown notice");
                    Err(anyhow::anyhow!("Relay notice: {}", notice_message))
                }
                Some(other) => {
                    Err(anyhow::anyhow!("Unexpected response type: {}", other))
                }
                None => {
                    Err(anyhow::anyhow!("Invalid response format"))
                }
            }
        } else {
            Err(anyhow::anyhow!("Response is not a JSON array"))
        }
    }
}