use anyhow::Result;
use secp256k1::{Secp256k1, SecretKey, PublicKey, Keypair};
use secp256k1::rand;

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

    pub fn public_key(&self) -> PublicKey {
        self.keypair.public_key()
    }

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
    // TODO: Consider upgrading to hardware-based true randomness for production use
    // Current implementation uses secp256k1's built-in CSPRNG which is cryptographically secure
    // but for maximum security, consider crates like:
    // - `rdrand` for Intel hardware random number generation
    // - `getrandom` with hardware entropy sources
    // - `ring::rand` for additional entropy mixing
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