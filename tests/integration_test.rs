mod mock_relay;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use nosotros::nostr::{NostrEvent, generate_keypair};
use mock_relay::MockRelay;

#[tokio::test]
async fn test_post_command_integration() -> Result<()> {
    println!("🚀 Starting integration test for post command");

    let mut relay = MockRelay::new().await?;
    let relay_url = relay.websocket_url();
    println!("📡 Mock relay started at: {}", relay_url);

    let keypair = generate_keypair()?;
    println!("🔑 Generated test keypair");

    let test_message = "Hello from integration test! 🧪";
    let event = NostrEvent::new_text_note(test_message.to_string(), &keypair)?;

    println!("📝 Created event:");
    println!("   ID: {}", event.id);
    println!("   Content: {}", event.content);
    println!("   Pubkey: {}", event.pubkey);

    let relay_task = tokio::spawn(async move {
        if let Err(e) = relay.start().await {
            eprintln!("Relay error: {}", e);
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let (mut ws_stream, _) = connect_async(&relay_url).await?;
    println!("🔗 Connected to mock relay");

    let event_json = event.to_json_value()?;
    let message = serde_json::json!(["EVENT", event_json]);
    let message_text = serde_json::to_string(&message)?;

    println!("📤 Sending EVENT message");
    ws_stream.send(Message::Text(message_text.into())).await?;

    let response = timeout(Duration::from_secs(5), ws_stream.next()).await?;

    if let Some(Ok(Message::Text(response_text))) = response {
        println!("📥 Received response: {}", response_text);

        let response_json: Value = serde_json::from_str(&response_text.to_string())?;

        if let Some(response_array) = response_json.as_array() {
            assert_eq!(response_array[0], "OK", "Expected OK response");
            assert_eq!(response_array[1], event.id, "Event ID should match");
            assert_eq!(response_array[2], true, "Event should be accepted");

            println!("✅ Event was successfully validated and accepted by mock relay");
        } else {
            panic!("Invalid response format");
        }
    } else {
        panic!("No response received from relay");
    }

    ws_stream.close(None).await?;
    relay_task.abort();

    println!("🎉 Integration test completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_invalid_event_rejection() -> Result<()> {
    println!("🚀 Starting invalid event rejection test");

    let mut relay = MockRelay::new().await?;
    let relay_url = relay.websocket_url();

    let keypair = generate_keypair()?;
    let mut event = NostrEvent::new_text_note("Test message".to_string(), &keypair)?;

    event.sig = "invalid_signature".to_string();
    println!("🔧 Corrupted event signature for testing");

    let relay_task = tokio::spawn(async move {
        if let Err(e) = relay.start().await {
            eprintln!("Relay error: {}", e);
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let (mut ws_stream, _) = connect_async(&relay_url).await?;

    let event_json = event.to_json_value()?;
    let message = serde_json::json!(["EVENT", event_json]);
    let message_text = serde_json::to_string(&message)?;

    ws_stream.send(Message::Text(message_text.into())).await?;

    let response = timeout(Duration::from_secs(5), ws_stream.next()).await?;

    if let Some(Ok(Message::Text(response_text))) = response {
        let response_json: Value = serde_json::from_str(&response_text.to_string())?;

        if let Some(response_array) = response_json.as_array() {
            assert_eq!(response_array[0], "OK");
            assert_eq!(response_array[1], event.id);
            assert_eq!(response_array[2], false, "Invalid event should be rejected");

            println!("✅ Invalid event was correctly rejected by mock relay");
        }
    }

    ws_stream.close(None).await?;
    relay_task.abort();

    println!("🎉 Invalid event rejection test completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_event_validation_components() -> Result<()> {
    println!("🚀 Testing individual event validation components");

    let keypair = generate_keypair()?;
    let event = NostrEvent::new_text_note("Component test message".to_string(), &keypair)?;

    assert!(!event.id.is_empty(), "Event ID should not be empty");
    assert_eq!(event.id.len(), 64, "Event ID should be 64 characters");

    assert!(!event.pubkey.is_empty(), "Public key should not be empty");
    assert_eq!(event.pubkey.len(), 64, "Public key should be 64 characters");

    assert!(!event.sig.is_empty(), "Signature should not be empty");
    assert_eq!(event.sig.len(), 128, "Signature should be 128 characters");

    assert_eq!(event.kind, 1, "Text note should have kind 1");
    assert_eq!(event.content, "Component test message");

    let is_valid = event.verify_signature(&keypair.public_key_hex())?;
    assert!(is_valid, "Event signature should be valid");

    println!("✅ All event validation components passed");
    Ok(())
}