use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce, Key,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedKeystore {
    pub salt: String,
    pub password_hash: String,
    pub nonce: Vec<u8>,
    pub encrypted_data: Vec<u8>,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct DecryptedKeys {
    pub keys: HashMap<String, SecretString>,
}

pub struct KeystoreManager {
    argon2: Argon2<'static>,
}

impl KeystoreManager {
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    pub fn create_keystore(
        &self,
        keys: &HashMap<String, String>,
        password: &SecretString,
    ) -> Result<EncryptedKeystore> {
        let salt = SaltString::generate(&mut OsRng);

        let password_hash = self
            .argon2
            .hash_password(password.expose_secret().as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?
            .to_string();

        let encryption_key = self.derive_encryption_key(password, &salt)?;

        let keys_json = serde_json::to_string(keys)
            .map_err(|e| anyhow!("Failed to serialize keys: {}", e))?;

        let cipher = ChaCha20Poly1305::new(&encryption_key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let encrypted_data = cipher
            .encrypt(&nonce, keys_json.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        Ok(EncryptedKeystore {
            salt: salt.to_string(),
            password_hash,
            nonce: nonce.to_vec(),
            encrypted_data,
            version: 1,
        })
    }

    pub fn decrypt_keystore(
        &self,
        keystore: &EncryptedKeystore,
        password: &SecretString,
    ) -> Result<DecryptedKeys> {
        self.verify_password(keystore, password)?;

        let salt = SaltString::from_b64(&keystore.salt)
            .map_err(|e| anyhow!("Invalid salt format: {}", e))?;

        let encryption_key = self.derive_encryption_key(password, &salt)?;

        let cipher = ChaCha20Poly1305::new(&encryption_key);
        let nonce = Nonce::from_slice(&keystore.nonce);
        let decrypted_data = cipher
            .decrypt(nonce, keystore.encrypted_data.as_slice())
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        let keys_json = String::from_utf8(decrypted_data)
            .map_err(|e| anyhow!("Invalid UTF-8 in decrypted data: {}", e))?;

        let keys_map: HashMap<String, String> = serde_json::from_str(&keys_json)
            .map_err(|e| anyhow!("Failed to parse decrypted keys: {}", e))?;

        let secure_keys: HashMap<String, SecretString> = keys_map
            .into_iter()
            .map(|(id, key)| (id, SecretString::new(key.into_boxed_str())))
            .collect();

        Ok(DecryptedKeys {
            keys: secure_keys,
        })
    }

    #[allow(dead_code)]
    pub fn add_key_to_keystore(
        &self,
        keystore: &EncryptedKeystore,
        password: &SecretString,
        account_id: &str,
        private_key: &str,
    ) -> Result<EncryptedKeystore> {
        let mut decrypted = self.decrypt_keystore(keystore, password)?;

        decrypted.keys.insert(account_id.to_string(), SecretString::new(private_key.to_string().into_boxed_str()));

        let keys_map: HashMap<String, String> = decrypted
            .keys
            .into_iter()
            .map(|(id, secret)| (id, secret.expose_secret().to_string()))
            .collect();

        self.create_keystore(&keys_map, password)
    }

    /// Remove a private key from existing keystore
    #[allow(dead_code)]
    pub fn remove_key_from_keystore(
        &self,
        keystore: &EncryptedKeystore,
        password: &SecretString,
        account_id: &str,
    ) -> Result<EncryptedKeystore> {
        let mut decrypted = self.decrypt_keystore(keystore, password)?;

        decrypted.keys.remove(account_id);

        let keys_map: HashMap<String, String> = decrypted
            .keys
            .into_iter()
            .map(|(id, secret)| (id, secret.expose_secret().to_string()))
            .collect();

        self.create_keystore(&keys_map, password)
    }

    fn verify_password(
        &self,
        keystore: &EncryptedKeystore,
        password: &SecretString,
    ) -> Result<()> {
        let parsed_hash = PasswordHash::new(&keystore.password_hash)
            .map_err(|e| anyhow!("Invalid password hash format: {}", e))?;

        self.argon2
            .verify_password(password.expose_secret().as_bytes(), &parsed_hash)
            .map_err(|_| anyhow!("Invalid password"))?;

        Ok(())
    }

    fn derive_encryption_key(
        &self,
        password: &SecretString,
        salt: &SaltString,
    ) -> Result<Key> {
        let password_hash = self
            .argon2
            .hash_password(password.expose_secret().as_bytes(), salt)
            .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

        // Extract the first 32 bytes of the hash for ChaCha20Poly1305 key
        let hash = password_hash.hash.unwrap();
        let hash_bytes = hash.as_bytes();
        if hash_bytes.len() < 32 {
            return Err(anyhow!("Derived hash too short for encryption key"));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&hash_bytes[..32]);

        Ok(*Key::from_slice(&key_bytes))
    }
}

impl DecryptedKeys {
    pub fn get_key(&self, account_id: &str) -> Option<&SecretString> {
        self.keys.get(account_id)
    }

    #[allow(dead_code)]
    pub fn has_key(&self, account_id: &str) -> bool {
        self.keys.contains_key(account_id)
    }

    #[allow(dead_code)]
    pub fn account_ids(&self) -> Vec<&String> {
        self.keys.keys().collect()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl Default for KeystoreManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_keystore_creation_and_decryption() {
        let manager = KeystoreManager::new();
        let password = SecretString::new("test_password_123".to_string().into_boxed_str());

        let mut keys = HashMap::new();
        keys.insert("account1".to_string(), "private_key_1".to_string());
        keys.insert("account2".to_string(), "private_key_2".to_string());

        let keystore = manager.create_keystore(&keys, &password).unwrap();

        let decrypted = manager.decrypt_keystore(&keystore, &password).unwrap();

        assert_eq!(decrypted.len(), 2);
        assert!(decrypted.has_key("account1"));
        assert!(decrypted.has_key("account2"));
        assert_eq!(decrypted.get_key("account1").unwrap().expose_secret(), "private_key_1");
        assert_eq!(decrypted.get_key("account2").unwrap().expose_secret(), "private_key_2");
    }

    #[test]
    fn test_wrong_password_fails() {
        let manager = KeystoreManager::new();
        let password = SecretString::new("correct_password".to_string().into_boxed_str());
        let wrong_password = SecretString::new("wrong_password".to_string().into_boxed_str());

        let mut keys = HashMap::new();
        keys.insert("account1".to_string(), "private_key_1".to_string());

        let keystore = manager.create_keystore(&keys, &password).unwrap();

        assert!(manager.decrypt_keystore(&keystore, &wrong_password).is_err());
    }

    #[test]
    fn test_add_key_to_keystore() {
        let manager = KeystoreManager::new();
        let password = SecretString::new("test_password".to_string().into_boxed_str());

        let mut keys = HashMap::new();
        keys.insert("account1".to_string(), "private_key_1".to_string());
        let keystore = manager.create_keystore(&keys, &password).unwrap();

        let updated_keystore = manager
            .add_key_to_keystore(&keystore, &password, "account2", "private_key_2")
            .unwrap();

        let decrypted = manager.decrypt_keystore(&updated_keystore, &password).unwrap();
        assert_eq!(decrypted.len(), 2);
        assert!(decrypted.has_key("account1"));
        assert!(decrypted.has_key("account2"));
    }

    #[test]
    fn test_remove_key_from_keystore() {
        let manager = KeystoreManager::new();
        let password = SecretString::new("test_password".to_string().into_boxed_str());

        let mut keys = HashMap::new();
        keys.insert("account1".to_string(), "private_key_1".to_string());
        keys.insert("account2".to_string(), "private_key_2".to_string());
        let keystore = manager.create_keystore(&keys, &password).unwrap();

        let updated_keystore = manager
            .remove_key_from_keystore(&keystore, &password, "account1")
            .unwrap();

        let decrypted = manager.decrypt_keystore(&updated_keystore, &password).unwrap();
        assert_eq!(decrypted.len(), 1);
        assert!(!decrypted.has_key("account1"));
        assert!(decrypted.has_key("account2"));
    }

    #[test]
    fn test_empty_keystore() {
        let manager = KeystoreManager::new();
        let password = SecretString::new("test_password".to_string().into_boxed_str());

        let keys = HashMap::new();
        let keystore = manager.create_keystore(&keys, &password).unwrap();
        let decrypted = manager.decrypt_keystore(&keystore, &password).unwrap();

        assert!(decrypted.is_empty());
        assert_eq!(decrypted.len(), 0);
    }
}