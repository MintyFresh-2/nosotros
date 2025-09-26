use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::nostr::keys::NostrKeypair;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnsignedEvent {
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

impl UnsignedEvent {
    pub fn new_text_note(content: String, pubkey: String) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            pubkey,
            created_at,
            kind: 1, // Text note
            tags: vec![],
            content,
        }
    }

    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.created_at = timestamp;
        self
    }

    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_kind(mut self, kind: u16) -> Self {
        self.kind = kind;
        self
    }

    pub fn calculate_id(&self) -> Result<String> {
        let serialized = serde_json::to_string(&[
            serde_json::Value::Number(0.into()),
            serde_json::Value::String(self.pubkey.clone()),
            serde_json::Value::Number(self.created_at.into()),
            serde_json::Value::Number(self.kind.into()),
            serde_json::to_value(&self.tags)?,
            serde_json::Value::String(self.content.clone()),
        ])?;

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();

        Ok(hex::encode(hash))
    }

    pub fn sign(self, keypair: &NostrKeypair) -> Result<NostrEvent> {
        let id = self.calculate_id()?;
        let id_bytes = hex::decode(&id)?;
        let signature = keypair.sign_message(&id_bytes)?;
        let sig = hex::encode(signature);

        Ok(NostrEvent {
            id,
            pubkey: self.pubkey,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags,
            content: self.content,
            sig,
        })
    }
}

impl NostrEvent {
    pub fn new_text_note(content: String, keypair: &NostrKeypair) -> Result<Self> {
        let unsigned = UnsignedEvent::new_text_note(content, keypair.public_key_hex());
        unsigned.sign(keypair)
    }


    pub fn verify_signature(&self, public_key_hex: &str) -> Result<bool> {
        let id_bytes = hex::decode(&self.id)?;
        let sig_bytes = hex::decode(&self.sig)?;
        let pubkey_bytes = hex::decode(public_key_hex)?;

        if sig_bytes.len() != 64 {
            return Ok(false);
        }

        if pubkey_bytes.len() != 32 {
            return Ok(false);
        }

        let secp = secp256k1::Secp256k1::new();

        let sig_array: [u8; 64] = sig_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid signature length"))?;
        let signature = secp256k1::schnorr::Signature::from_byte_array(sig_array);

        let pubkey_array: [u8; 32] = pubkey_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
        let x_only_pubkey = secp256k1::XOnlyPublicKey::from_byte_array(pubkey_array)?;

        let id_array: [u8; 32] = id_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid message length"))?;

        match secp.verify_schnorr(&signature, &id_array, &x_only_pubkey) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_json_value(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nostr::keys;

    #[test]
    fn test_event_creation_and_verification() {
        let keypair = keys::generate_keypair().unwrap();
        let event = NostrEvent::new_text_note("Hello, Nostr!".to_string(), &keypair).unwrap();

        assert_eq!(event.kind, 1);
        assert_eq!(event.content, "Hello, Nostr!");
        assert_eq!(event.pubkey, keypair.public_key_hex());
        assert!(!event.id.is_empty());
        assert!(!event.sig.is_empty());

        let is_valid = event.verify_signature(&keypair.public_key_hex()).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_json_serialization() {
        let keypair = keys::generate_keypair().unwrap();
        let event = NostrEvent::new_text_note("Test message".to_string(), &keypair).unwrap();

        let json = event.to_json().unwrap();
        let parsed: NostrEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, parsed.id);
        assert_eq!(event.content, parsed.content);
        assert_eq!(event.sig, parsed.sig);
    }

    #[test]
    fn test_immutable_event_creation() {
        let keypair = keys::generate_keypair().unwrap();
        let pubkey = keypair.public_key_hex();

        // Create unsigned event
        let unsigned = UnsignedEvent::new_text_note("Immutable test".to_string(), pubkey.clone())
            .with_timestamp(1234567890)
            .with_tags(vec![vec!["t".to_string(), "test".to_string()]]);

        // Verify unsigned event properties
        assert_eq!(unsigned.content, "Immutable test");
        assert_eq!(unsigned.pubkey, pubkey);
        assert_eq!(unsigned.created_at, 1234567890);
        assert_eq!(unsigned.kind, 1);
        assert_eq!(unsigned.tags, vec![vec!["t".to_string(), "test".to_string()]]);

        // Calculate ID before signing
        let expected_id = unsigned.calculate_id().unwrap();

        // Sign to create immutable event
        let signed = unsigned.sign(&keypair).unwrap();

        // Verify signed event properties
        assert_eq!(signed.id, expected_id);
        assert_eq!(signed.content, "Immutable test");
        assert_eq!(signed.pubkey, pubkey);
        assert!(!signed.sig.is_empty());

        // Verify signature
        let is_valid = signed.verify_signature(&pubkey).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_unsigned_event_id_calculation() {
        let pubkey = "test_pubkey".to_string();
        let unsigned = UnsignedEvent::new_text_note("Test content".to_string(), pubkey)
            .with_timestamp(1234567890);

        let id1 = unsigned.calculate_id().unwrap();
        let id2 = unsigned.calculate_id().unwrap();

        // ID calculation should be deterministic
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64); // SHA256 hex = 64 chars
    }
}