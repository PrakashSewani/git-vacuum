use std::sync::Mutex;

use git_vacuum_core::{KeyringError, KeyringStore};

const SERVICE: &str = "git-vacuum";
const ACCOUNT: &str = "github-pat";

pub struct PlatformKeyring {
    entry: Mutex<Option<keyring::Entry>>,
}

impl PlatformKeyring {
    pub fn new() -> Result<Self, KeyringError> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(map_keyring_error)?;
        Ok(Self {
            entry: Mutex::new(Some(entry)),
        })
    }
}

impl KeyringStore for PlatformKeyring {
    fn set_token(&self, token: &str) -> Result<(), KeyringError> {
        let guard = self.entry.lock().expect("keyring mutex poisoned");
        let entry = guard.as_ref().ok_or(KeyringError::NoBackend)?;
        entry.set_password(token).map_err(map_keyring_error)
    }

    fn get_token(&self) -> Result<Option<String>, KeyringError> {
        let guard = self.entry.lock().expect("keyring mutex poisoned");
        let entry = guard.as_ref().ok_or(KeyringError::NoBackend)?;
        match entry.get_password() {
            Ok(t) => Ok(Some(t)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_keyring_error(e)),
        }
    }

    fn delete_token(&self) -> Result<(), KeyringError> {
        let guard = self.entry.lock().expect("keyring mutex poisoned");
        let entry = guard.as_ref().ok_or(KeyringError::NoBackend)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_keyring_error(e)),
        }
    }
}

fn map_keyring_error(e: keyring::Error) -> KeyringError {
    let s = e.to_string();
    let lower = s.to_lowercase();
    if lower.contains("no backend")
        || lower.contains("no platform")
        || lower.contains("platform not supported")
        || lower.contains("dbus")
        || lower.contains("no secret service")
    {
        return KeyringError::NoBackend;
    }
    match e {
        keyring::Error::NoEntry => KeyringError::NoEntry,
        other => KeyringError::Platform(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_token() {
        let kr = PlatformKeyring::new();
        if kr.is_err() {
            eprintln!("skipping keyring test: no platform backend (likely headless CI)");
            return;
        }
        let kr = kr.unwrap();

        let _ = kr.delete_token();
        kr.set_token("ghp_test_round_trip_xxxxxxxxxxxxxxxxxxxx")
            .expect("set");
        let got = kr.get_token().expect("get");
        assert_eq!(
            got.as_deref(),
            Some("ghp_test_round_trip_xxxxxxxxxxxxxxxxxxxx")
        );
        kr.delete_token().expect("delete");
        let after = kr.get_token().expect("get after delete");
        assert!(after.is_none());
    }
}
