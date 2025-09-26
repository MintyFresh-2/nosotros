use nosotros::nostr::{NostrEvent, generate_keypair};
use anyhow::Result;

#[tokio::test]
async fn debug_signature_verification() -> Result<()> {
    let keypair = generate_keypair()?;
    let event = NostrEvent::new_text_note("Debug test".to_string(), &keypair)?;

    println!("Event ID: {}", event.id);
    println!("Event pubkey: {} (len: {})", event.pubkey, event.pubkey.len());
    println!("Event signature: {} (len: {})", event.sig, event.sig.len());
    println!("Keypair pubkey: {} (len: {})", keypair.public_key_hex(), keypair.public_key_hex().len());

    // Verify using the event's built-in method
    let is_valid = event.verify_signature(&keypair.public_key_hex())?;
    println!("Verification result: {}", is_valid);

    // Also verify that the public keys match
    assert_eq!(event.pubkey, keypair.public_key_hex(), "Public keys should match");

    Ok(())
}