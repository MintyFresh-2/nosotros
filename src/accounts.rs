use anyhow::{Result, anyhow};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::keystore::{DecryptedKeys, EncryptedKeystore, KeystoreManager};
use crate::nostr::{NostrKeypair, generate_keypair, keypair_from_hex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub public_key_hex: String,
    pub public_key_npub: String,
    pub created_at: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountsConfig {
    pub accounts: Vec<AccountInfo>,
    pub active_account_id: Option<String>,
    pub security_settings: SecuritySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub require_auth_for_signing: bool,
    pub auto_lock_timeout_minutes: Option<u32>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UnlockedAccount {
    pub info: AccountInfo,
    pub keypair: NostrKeypair,
}

pub struct AccountManager {
    config_dir: PathBuf,
    keystore_manager: KeystoreManager,
    accounts_config: AccountsConfig,
    unlocked_keys: Option<DecryptedKeys>,
}

#[allow(dead_code)]
impl AccountManager {
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&config_dir)?;

        let keystore_manager = KeystoreManager::new();
        let accounts_config = Self::load_accounts_config(&config_dir)?;

        Ok(Self {
            config_dir,
            keystore_manager,
            accounts_config,
            unlocked_keys: None,
        })
    }

    /// Unlock the keystore with a password, allowing access to private keys
    pub fn unlock_keystore(&mut self, password: &SecretString) -> Result<()> {
        let keystore_path = self.keystore_path();

        if !keystore_path.exists() {
            let empty_keys = HashMap::new();
            let keystore = self
                .keystore_manager
                .create_keystore(&empty_keys, password)?;
            self.save_keystore(&keystore)?;
            self.unlocked_keys = Some(DecryptedKeys {
                keys: HashMap::new(),
            });
            return Ok(());
        }

        let keystore = self.load_keystore()?;
        let decrypted_keys = self
            .keystore_manager
            .decrypt_keystore(&keystore, password)?;
        self.unlocked_keys = Some(decrypted_keys);
        Ok(())
    }

    pub fn lock_keystore(&mut self) {
        self.unlocked_keys = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_keys.is_some()
    }

    pub fn create_account(&mut self, name: &str, password: &SecretString) -> Result<AccountInfo> {
        if !self.is_unlocked() {
            self.unlock_keystore(password)?;
        }

        let keypair = generate_keypair()?;
        let account_id = Uuid::new_v4().to_string();

        let account_info = AccountInfo {
            id: account_id.clone(),
            name: name.to_string(),
            public_key_hex: keypair.public_key_hex(),
            public_key_npub: keypair.public_key_npub()?,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_active: self.accounts_config.accounts.is_empty(), // First account is active by default
        };

        self.add_private_key_to_keystore(&account_id, &keypair.secret_key_hex(), password)?;

        self.accounts_config.accounts.push(account_info.clone());

        if self.accounts_config.active_account_id.is_none() {
            self.accounts_config.active_account_id = Some(account_id);
        }

        self.save_accounts_config()?;

        Ok(account_info)
    }

    pub fn import_account(
        &mut self,
        name: &str,
        private_key_hex: &str,
        password: &SecretString,
    ) -> Result<AccountInfo> {
        if !self.is_unlocked() {
            self.unlock_keystore(password)?;
        }

        let keypair = keypair_from_hex(private_key_hex)?;
        let account_id = Uuid::new_v4().to_string();

        let public_key_hex = keypair.public_key_hex();
        if self
            .accounts_config
            .accounts
            .iter()
            .any(|acc| acc.public_key_hex == public_key_hex)
        {
            return Err(anyhow!("Account with this public key already exists"));
        }

        let account_info = AccountInfo {
            id: account_id.clone(),
            name: name.to_string(),
            public_key_hex,
            public_key_npub: keypair.public_key_npub()?,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_active: self.accounts_config.accounts.is_empty(),
        };

        self.add_private_key_to_keystore(&account_id, private_key_hex, password)?;

        self.accounts_config.accounts.push(account_info.clone());

        if self.accounts_config.active_account_id.is_none() {
            self.accounts_config.active_account_id = Some(account_id);
        }

        self.save_accounts_config()?;

        Ok(account_info)
    }

    pub fn delete_account(&mut self, account_id: &str, password: &SecretString) -> Result<()> {
        if !self.is_unlocked() {
            self.unlock_keystore(password)?;
        }

        let account_index = self
            .accounts_config
            .accounts
            .iter()
            .position(|acc| acc.id == account_id)
            .ok_or_else(|| anyhow!("Account not found"))?;

        let was_active =
            self.accounts_config.active_account_id.as_ref() == Some(&account_id.to_string());
        self.accounts_config.accounts.remove(account_index);

        self.remove_private_key_from_keystore(account_id, password)?;

        if was_active {
            self.accounts_config.active_account_id = self
                .accounts_config
                .accounts
                .first()
                .map(|acc| acc.id.clone());
        }

        self.save_accounts_config()?;

        Ok(())
    }

    pub fn set_active_account(&mut self, account_id: &str) -> Result<()> {
        if !self
            .accounts_config
            .accounts
            .iter()
            .any(|acc| acc.id == account_id)
        {
            return Err(anyhow!("Account not found"));
        }

        for account in &mut self.accounts_config.accounts {
            account.is_active = account.id == account_id;
        }

        self.accounts_config.active_account_id = Some(account_id.to_string());
        self.save_accounts_config()?;

        Ok(())
    }

    pub fn get_active_account(&self) -> Result<Option<UnlockedAccount>> {
        let unlocked_keys = self
            .unlocked_keys
            .as_ref()
            .ok_or_else(|| anyhow!("Keystore is locked"))?;

        let active_id = match &self.accounts_config.active_account_id {
            Some(id) => id,
            None => return Ok(None),
        };

        let account_info = self
            .accounts_config
            .accounts
            .iter()
            .find(|acc| acc.id == *active_id)
            .ok_or_else(|| anyhow!("Active account not found in config"))?;

        let private_key = unlocked_keys
            .get_key(active_id)
            .ok_or_else(|| anyhow!("Private key not found for active account"))?;

        let keypair = keypair_from_hex(private_key.expose_secret())?;

        Ok(Some(UnlockedAccount {
            info: account_info.clone(),
            keypair,
        }))
    }

    pub fn get_account(&self, account_id: &str) -> Result<Option<UnlockedAccount>> {
        let unlocked_keys = self
            .unlocked_keys
            .as_ref()
            .ok_or_else(|| anyhow!("Keystore is locked"))?;

        let account_info = match self
            .accounts_config
            .accounts
            .iter()
            .find(|acc| acc.id == account_id)
        {
            Some(info) => info,
            None => return Ok(None),
        };

        let private_key = match unlocked_keys.get_key(account_id) {
            Some(key) => key,
            None => return Ok(None),
        };

        let keypair = keypair_from_hex(private_key.expose_secret())?;

        Ok(Some(UnlockedAccount {
            info: account_info.clone(),
            keypair,
        }))
    }

    pub fn list_accounts(&self) -> &[AccountInfo] {
        &self.accounts_config.accounts
    }

    pub fn active_account_id(&self) -> Option<&String> {
        self.accounts_config.active_account_id.as_ref()
    }

    fn add_private_key_to_keystore(
        &mut self,
        account_id: &str,
        private_key_hex: &str,
        password: &SecretString,
    ) -> Result<()> {
        let keystore = self.load_keystore()?;
        let updated_keystore = self.keystore_manager.add_key_to_keystore(
            &keystore,
            password,
            account_id,
            private_key_hex,
        )?;
        self.save_keystore(&updated_keystore)?;

        if let Some(ref mut unlocked) = self.unlocked_keys {
            unlocked.keys.insert(
                account_id.to_string(),
                SecretString::new(private_key_hex.to_string().into_boxed_str()),
            );
        }

        Ok(())
    }

    fn remove_private_key_from_keystore(
        &mut self,
        account_id: &str,
        password: &SecretString,
    ) -> Result<()> {
        let keystore = self.load_keystore()?;
        let updated_keystore = self
            .keystore_manager
            .remove_key_from_keystore(&keystore, password, account_id)?;
        self.save_keystore(&updated_keystore)?;

        if let Some(ref mut unlocked) = self.unlocked_keys {
            unlocked.keys.remove(account_id);
        }

        Ok(())
    }

    fn load_accounts_config(config_dir: &Path) -> Result<AccountsConfig> {
        let config_path = config_dir.join("accounts.json");

        if !config_path.exists() {
            let default_config = AccountsConfig {
                accounts: Vec::new(),
                active_account_id: None,
                security_settings: SecuritySettings {
                    require_auth_for_signing: true,
                    auto_lock_timeout_minutes: Some(30),
                },
            };

            let config_json = serde_json::to_string_pretty(&default_config)?;
            fs::write(&config_path, config_json)?;

            return Ok(default_config);
        }

        let config_json = fs::read_to_string(&config_path)?;
        let config: AccountsConfig = serde_json::from_str(&config_json)?;
        Ok(config)
    }

    fn save_accounts_config(&self) -> Result<()> {
        let config_path = self.config_dir.join("accounts.json");
        let config_json = serde_json::to_string_pretty(&self.accounts_config)?;
        fs::write(&config_path, config_json)?;
        Ok(())
    }

    fn load_keystore(&self) -> Result<EncryptedKeystore> {
        let keystore_path = self.keystore_path();
        let keystore_json = fs::read_to_string(&keystore_path)?;
        let keystore: EncryptedKeystore = serde_json::from_str(&keystore_json)?;
        Ok(keystore)
    }

    fn save_keystore(&self, keystore: &EncryptedKeystore) -> Result<()> {
        let keystore_path = self.keystore_path();
        let keystore_json = serde_json::to_string_pretty(keystore)?;
        fs::write(&keystore_path, keystore_json)?;
        Ok(())
    }

    fn keystore_path(&self) -> PathBuf {
        self.config_dir.join("keystore.json")
    }
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            require_auth_for_signing: true,
            auto_lock_timeout_minutes: Some(30),
        }
    }
}
