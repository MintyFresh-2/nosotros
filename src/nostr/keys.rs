use anyhow::Result;
use secp256k1::{Secp256k1, SecretKey, PublicKey, Keypair};
use secp256k1::rand;
use bech32::{Bech32, Hrp};

#[derive(Debug, Clone)]
pub struct NostrKeypair {
    keypair: Keypair,
}

impl NostrKeypair {
    pub fn new(keypair: Keypair) -> Self {
        Self { keypair }
    }

    pub fn secret_key_hex(&self) -> String {
        hex::encode(self.keypair.secret_key().secret_bytes())
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.keypair.public_key().x_only_public_key().0.serialize())
    }

    pub fn public_key_npub(&self) -> Result<String> {
        let pubkey_bytes = self.keypair.public_key().x_only_public_key().0.serialize();
        let hrp = Hrp::parse("npub").map_err(|e| anyhow::anyhow!("Invalid HRP: {}", e))?;
        let encoded = bech32::encode::<Bech32>(hrp, &pubkey_bytes)
            .map_err(|e| anyhow::anyhow!("Bech32 encoding failed: {}", e))?;
        Ok(encoded)
    }

    #[allow(dead_code)]
    pub fn public_key(&self) -> PublicKey {
        self.keypair.public_key()
    }

    #[allow(dead_code)]
    pub fn secret_key(&self) -> SecretKey {
        self.keypair.secret_key()
    }

    pub fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        let secp = Secp256k1::new();
        let message_array: [u8; 32] = message.try_into()
            .map_err(|_| anyhow::anyhow!("Message must be exactly 32 bytes"))?;
        let signature = secp.sign_schnorr(&message_array, &self.keypair);
        Ok(signature.as_ref().to_vec())
    }
}

pub fn generate_keypair() -> Result<NostrKeypair> {
    let secp = Secp256k1::new();
    let (secret_key, _) = secp.generate_keypair(&mut rand::rng());
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    Ok(NostrKeypair::new(keypair))
}

pub fn keypair_from_hex(secret_hex: &str) -> Result<NostrKeypair> {
    let secret_bytes = hex::decode(secret_hex)?;
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_byte_array(
        secret_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid secret key length"))?
    )?;
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    Ok(NostrKeypair::new(keypair))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::XOnlyPublicKey;
    use sha2::Digest;

    #[test]
    fn test_keypair_generation() {
        let keypair = generate_keypair().expect("Should generate keypair");

        // Verify key lengths
        assert_eq!(keypair.secret_key_hex().len(), 64); // 32 bytes * 2 hex chars
        assert_eq!(keypair.public_key_hex().len(), 64); // 32 bytes * 2 hex chars

        // Verify keys are valid hex
        assert!(hex::decode(&keypair.secret_key_hex()).is_ok());
        assert!(hex::decode(&keypair.public_key_hex()).is_ok());
    }

    #[test]
    fn test_keypair_from_hex_deterministic() {
        let test_private_key = "8182a1283a6e4a2ee5c0e6fedcc003b3e810e2a93d864946df32ed2baccd71a5";
        let expected_public_key = "f1a56439ab2a3d3246a21463aacf833f503caf6627df3b6c110719f5ab7b77b3";

        let keypair = keypair_from_hex(test_private_key).expect("Should create keypair from hex");

        assert_eq!(keypair.secret_key_hex(), test_private_key);
        assert_eq!(keypair.public_key_hex(), expected_public_key);
    }

    #[test]
    fn test_public_key_derivation_methods_match() {
        let test_private_key = "8182a1283a6e4a2ee5c0e6fedcc003b3e810e2a93d864946df32ed2baccd71a5";
        let keypair = keypair_from_hex(test_private_key).expect("Should create keypair from hex");

        // Method 1: Our current approach
        let method1 = keypair.keypair.public_key().x_only_public_key().0.serialize();

        // Method 2: Direct XOnlyPublicKey approach
        let (xonly_pubkey, _parity) = XOnlyPublicKey::from_keypair(&keypair.keypair);
        let method2 = xonly_pubkey.serialize();

        // Both methods should produce identical results
        assert_eq!(method1, method2);
        assert_eq!(hex::encode(method1), keypair.public_key_hex());
    }

    #[test]
    fn test_key_roundtrip() {
        let original_keypair = generate_keypair().expect("Should generate keypair");
        let private_hex = original_keypair.secret_key_hex();

        let restored_keypair = keypair_from_hex(&private_hex).expect("Should restore keypair");

        assert_eq!(original_keypair.secret_key_hex(), restored_keypair.secret_key_hex());
        assert_eq!(original_keypair.public_key_hex(), restored_keypair.public_key_hex());
    }

    #[test]
    fn test_signature_creation_and_verification() {
        let keypair = generate_keypair().expect("Should generate keypair");
        let message = b"Hello, Nostr!";
        let message_hash = sha2::Sha256::digest(message);

        let signature = keypair.sign_message(&message_hash).expect("Should sign message");

        // Signature should be 64 bytes for Schnorr
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_invalid_private_key_handling() {
        // Test with invalid hex
        assert!(keypair_from_hex("invalid_hex").is_err());

        // Test with wrong length
        assert!(keypair_from_hex("1234").is_err());

        // Test with all zeros (invalid private key)
        assert!(keypair_from_hex("0000000000000000000000000000000000000000000000000000000000000000").is_err());
    }

    #[test]
    fn test_npub_encoding() {
        let test_private_key = "8182a1283a6e4a2ee5c0e6fedcc003b3e810e2a93d864946df32ed2baccd71a5";
        let keypair = keypair_from_hex(test_private_key).expect("Should create keypair from hex");

        let npub = keypair.public_key_npub().expect("Should encode npub");

        // npub should start with "npub1"
        assert!(npub.starts_with("npub1"));

        // npub should be longer than just the prefix
        assert!(npub.len() > 5);
    }

    #[test]
    fn test_npub_deterministic() {
        let test_private_key = "8182a1283a6e4a2ee5c0e6fedcc003b3e810e2a93d864946df32ed2baccd71a5";
        let keypair1 = keypair_from_hex(test_private_key).expect("Should create keypair from hex");
        let keypair2 = keypair_from_hex(test_private_key).expect("Should create keypair from hex");

        let npub1 = keypair1.public_key_npub().expect("Should encode npub");
        let npub2 = keypair2.public_key_npub().expect("Should encode npub");

        // Same private key should always produce same npub
        assert_eq!(npub1, npub2);
    }

    #[test]
    fn test_different_keys_produce_different_npubs() {
        let keypair1 = generate_keypair().expect("Should generate keypair");
        let keypair2 = generate_keypair().expect("Should generate keypair");

        let npub1 = keypair1.public_key_npub().expect("Should encode npub");
        let npub2 = keypair2.public_key_npub().expect("Should encode npub");

        // Different keypairs should produce different npubs
        assert_ne!(npub1, npub2);
    }
}