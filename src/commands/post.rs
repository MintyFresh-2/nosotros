use anyhow::Result;
use futures_util::SinkExt;
use serde_json;
use tokio_tungstenite::tungstenite::Message;

use crate::connection::RelayManager;
use crate::nostr::{keypair_from_hex, NostrEvent};

pub struct PostCommand {
    pub text: String,
    pub relay_url: String,
    pub private_key_hex: String,
}

impl PostCommand {
    pub fn new(text: String, relay_url: String, private_key_hex: String) -> Self {
        Self {
            text,
            relay_url,
            private_key_hex,
        }
    }

    pub async fn execute(&self) -> Result<String> {
        println!("Creating and posting event: {}", self.text);

        // Load keypair from hex
        let keypair = keypair_from_hex(&self.private_key_hex)
            .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))?;

        // Create event
        let event = NostrEvent::new_text_note(self.text.clone(), &keypair)
            .map_err(|e| anyhow::anyhow!("Failed to create event: {}", e))?;

        println!("Created event with ID: {}", event.id);
        println!("Public key: {}", event.pubkey);

        // Connect to relay and publish
        let mut relay_manager = RelayManager::new();
        relay_manager.add_relay(&self.relay_url).await
            .map_err(|e| anyhow::anyhow!("Invalid relay URL: {}", e))?;

        let mut connection = relay_manager.connect_relay(&self.relay_url).await
            .map_err(|e| anyhow::anyhow!("Failed to connect to relay: {}", e))?;

        println!("Connected to relay: {}", self.relay_url);

        // Create EVENT message
        let event_json = event.to_json_value()?;
        let message = serde_json::json!(["EVENT", event_json]);
        let message_text = serde_json::to_string(&message)?;

        // Send to relay
        connection.send(Message::Text(message_text.into())).await
            .map_err(|e| anyhow::anyhow!("Failed to send event to relay: {}", e))?;

        println!("✅ Event published successfully!");
        println!("Event ID: {}", event.id);

        Ok(event.id)
    }
}