use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

use nosotros::nostr::NostrEvent;

pub struct MockRelay {
    listener: TcpListener,
    addr: SocketAddr,
    events_received: Vec<NostrEvent>,
}

impl MockRelay {
    pub async fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        Ok(Self {
            listener,
            addr,
            events_received: Vec::new(),
        })
    }

    pub fn websocket_url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub async fn start(&mut self) -> Result<()> {
        println!("Mock relay listening on {}", self.addr);

        while let Ok((stream, _)) = self.listener.accept().await {
            let ws_stream = accept_async(stream).await?;
            self.handle_connection(ws_stream).await?;
            break; // Handle one connection for testing
        }

        Ok(())
    }

    async fn handle_connection(&mut self, mut ws_stream: WebSocketStream<TcpStream>) -> Result<()> {
        println!("New WebSocket connection established");

        while let Some(msg) = ws_stream.next().await {
            match msg? {
                Message::Text(text) => {
                    println!("Received message: {}", text);

                    match self.process_message(&text).await {
                        Ok(response) => {
                            if let Some(resp) = response {
                                let response_text = serde_json::to_string(&resp)?;
                                println!("Sending response: {}", response_text);
                                ws_stream.send(Message::Text(response_text.into())).await?;
                            }
                        }
                        Err(e) => {
                            println!("Error processing message: {}", e);
                            let error_response = json!(["NOTICE", format!("Error: {}", e)]);
                            let error_text = serde_json::to_string(&error_response)?;
                            ws_stream.send(Message::Text(error_text.into())).await?;
                        }
                    }
                }
                Message::Close(_) => {
                    println!("Connection closed");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn process_message(&mut self, message: &str) -> Result<Option<Value>> {
        let parsed: Value = serde_json::from_str(message)?;

        if let Some(array) = parsed.as_array() {
            if array.is_empty() {
                return Ok(None);
            }

            match array[0].as_str() {
                Some("EVENT") => {
                    if array.len() >= 2 {
                        return self.handle_event(&array[1]).await;
                    }
                }
                Some("REQ") => {
                    if array.len() >= 3 {
                        let subscription_id = array[1].as_str().unwrap_or("unknown");
                        return Ok(Some(json!(["EOSE", subscription_id])));
                    }
                }
                Some("CLOSE") => {
                    if array.len() >= 2 {
                        let subscription_id = array[1].as_str().unwrap_or("unknown");
                        return Ok(Some(json!(["CLOSED", subscription_id, ""])));
                    }
                }
                _ => {
                    return Ok(Some(json!(["NOTICE", "Unknown message type"])));
                }
            }
        }

        Ok(None)
    }

    async fn handle_event(&mut self, event_data: &Value) -> Result<Option<Value>> {
        println!("Processing EVENT: {}", serde_json::to_string_pretty(event_data)?);

        let event: NostrEvent = serde_json::from_value(event_data.clone())?;

        let validation_result = self.validate_event(&event).await;

        match validation_result {
            Ok(()) => {
                println!("✅ Event validation successful");
                self.events_received.push(event.clone());

                Ok(Some(json!([
                    "OK",
                    event.id,
                    true,
                    "Event accepted"
                ])))
            }
            Err(e) => {
                println!("❌ Event validation failed: {}", e);

                Ok(Some(json!([
                    "OK",
                    event.id,
                    false,
                    format!("Event rejected: {}", e)
                ])))
            }
        }
    }

    async fn validate_event(&self, event: &NostrEvent) -> Result<()> {
        println!("🔍 Validating event...");

        if event.id.is_empty() {
            return Err(anyhow::anyhow!("Event ID is empty"));
        }

        if event.pubkey.is_empty() {
            return Err(anyhow::anyhow!("Public key is empty"));
        }

        if event.sig.is_empty() {
            return Err(anyhow::anyhow!("Signature is empty"));
        }

        if event.id.len() != 64 {
            return Err(anyhow::anyhow!("Invalid event ID length: expected 64 chars, got {}", event.id.len()));
        }

        if event.pubkey.len() != 64 {
            return Err(anyhow::anyhow!("Invalid public key length: expected 64 chars, got {}", event.pubkey.len()));
        }

        if event.sig.len() != 128 {
            return Err(anyhow::anyhow!("Invalid signature length: expected 128 chars, got {}", event.sig.len()));
        }

        println!("📋 Basic validation passed");

        if let Err(e) = self.verify_event_id(event) {
            return Err(anyhow::anyhow!("Event ID verification failed: {}", e));
        }
        println!("🆔 Event ID verification passed");

        if let Err(e) = self.verify_signature(event) {
            return Err(anyhow::anyhow!("Signature verification failed: {}", e));
        }
        println!("✍️  Signature verification passed");

        Ok(())
    }

    fn verify_event_id(&self, event: &NostrEvent) -> Result<()> {
        use sha2::{Digest, Sha256};

        let serialized = serde_json::to_string(&[
            serde_json::Value::Number(0.into()),
            serde_json::Value::String(event.pubkey.clone()),
            serde_json::Value::Number(event.created_at.into()),
            serde_json::Value::Number(event.kind.into()),
            serde_json::to_value(&event.tags)?,
            serde_json::Value::String(event.content.clone()),
        ])?;

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();
        let computed_id = hex::encode(hash);

        if computed_id != event.id {
            return Err(anyhow::anyhow!(
                "Event ID mismatch: expected {}, got {}",
                computed_id,
                event.id
            ));
        }

        Ok(())
    }

    fn verify_signature(&self, event: &NostrEvent) -> Result<()> {
        let id_bytes = hex::decode(&event.id)?;
        let sig_bytes = hex::decode(&event.sig)?;
        let pubkey_bytes = hex::decode(&event.pubkey)?;

        if sig_bytes.len() != 64 {
            return Err(anyhow::anyhow!("Invalid signature length"));
        }

        if pubkey_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Invalid public key length"));
        }

        let secp = secp256k1::Secp256k1::new();
        let sig_array: [u8; 64] = sig_bytes.try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signature format"))?;
        let signature = secp256k1::schnorr::Signature::from_byte_array(sig_array);

        let pubkey_array: [u8; 32] = pubkey_bytes.try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public key format"))?;
        let x_only_pubkey = secp256k1::XOnlyPublicKey::from_byte_array(pubkey_array)?;

        let id_array: [u8; 32] = id_bytes.try_into()
            .map_err(|_| anyhow::anyhow!("Invalid message length"))?;

        match secp.verify_schnorr(&signature, &id_array, &x_only_pubkey) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Signature verification failed: {}", e)),
        }
    }

    pub fn events_received(&self) -> &[NostrEvent] {
        &self.events_received
    }

    pub fn event_count(&self) -> usize {
        self.events_received.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nosotros::nostr;

    #[tokio::test]
    async fn test_mock_relay_basic() -> Result<()> {
        let mut relay = MockRelay::new().await?;
        println!("Mock relay created on {}", relay.websocket_url());
        assert!(relay.port() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_event_validation() -> Result<()> {
        let relay = MockRelay::new().await?;
        let keypair = nostr::generate_keypair()?;
        let event = nosotros::nostr::NostrEvent::new_text_note(
            "Test message for validation".to_string(),
            &keypair
        )?;

        let validation_result = relay.validate_event(&event).await;
        assert!(validation_result.is_ok(), "Event validation should pass: {:?}", validation_result);

        Ok(())
    }
}