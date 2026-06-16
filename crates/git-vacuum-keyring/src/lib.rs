use std::collections::HashMap;
use std::sync::Mutex;

use git_vacuum_core::traits::KeyringStore;

pub struct PlatformKeyring {
    fallback_store: Mutex<HashMap<String, String>>,
}

impl PlatformKeyring {
    pub fn new() -> Self {
        Self {
            fallback_store: Mutex::new(HashMap::new()),
        }
    }

    fn make_key(service: &str, username: &str) -> String {
        format!("{}:{}", service, username)
    }
}

#[async_trait::async_trait]
impl KeyringStore for PlatformKeyring {
    async fn set_token(&self, service: &str, username: &str, token: &str) -> Result<(), String> {
        match keyring::Entry::new(service, username) {
            Ok(entry) => entry.set_password(token).map_err(|e| format!("Keyring error: {}", e)),
            Err(_e) => {
                let key = Self::make_key(service, username);
                let mut store = self.fallback_store.lock().map_err(|e| format!("Lock error: {}", e))?;
                store.insert(key, token.to_string());
                Ok(())
            }
        }
    }

    async fn get_token(&self, service: &str, username: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(service, username) {
            Ok(entry) => match entry.get_password() {
                Ok(token) => Ok(Some(token)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(format!("Keyring error: {}", e)),
            },
            Err(_) => {
                let key = Self::make_key(service, username);
                let store = self.fallback_store.lock().map_err(|e| format!("Lock error: {}", e))?;
                Ok(store.get(&key).cloned())
            }
        }
    }

    async fn delete_token(&self, service: &str, username: &str) -> Result<(), String> {
        let key = Self::make_key(service, username);
        let mut store = self.fallback_store.lock().map_err(|e| format!("Lock error: {}", e))?;
        store.remove(&key);
        Ok(())
    }
}
