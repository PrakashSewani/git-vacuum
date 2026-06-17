use std::sync::Arc;

use git_vacuum_core::{AuthError, KeyringError, UserInfo};

use crate::Services;

/// Validate a PAT against GitHub, then persist it to the OS keyring
/// and upsert the account metadata in the database.
/// Token goes to keyring ONLY (never to SQLite, never logged).
pub async fn authenticate_pat(
    services: Arc<Services>,
    token: &str,
) -> Result<UserInfo, AuthError> {
    if token.is_empty() {
        return Err(AuthError::InvalidToken);
    }
    services.github.set_token(token);
    let info = services.github.validate_token().await?;
    services
        .keyring
        .set_token(token)
        .map_err(map_keyring_error)?;
    services
        .db
        .upsert_account(&info)
        .map_err(|e| AuthError::Internal(format!("db upsert: {e}")))?;
    Ok(info)
}

/// Load the stored token from the keyring, validate it, and return the user info.
/// On 401, clear the keyring entry and return InvalidToken.
pub async fn load_stored_credentials(
    services: Arc<Services>,
) -> Result<Option<UserInfo>, AuthError> {
    let token = match services.keyring.get_token() {
        Ok(Some(t)) => t,
        Ok(None) => return Ok(None),
        Err(KeyringError::NoEntry) => return Ok(None),
        Err(e) => return Err(AuthError::Internal(format!("keyring: {e}"))),
    };
    services.github.set_token(&token);
    match services.github.validate_token().await {
        Ok(info) => {
            let _ = services.db.upsert_account(&info);
            Ok(Some(info))
        }
        Err(AuthError::InvalidToken) | Err(AuthError::ExpiredToken) => {
            let _ = services.keyring.delete_token();
            let _ = services.db.clear_active_account();
            Err(AuthError::InvalidToken)
        }
        Err(e) => Err(e),
    }
}

pub async fn logout(services: Arc<Services>) -> Result<(), KeyringError> {
    services.github.clear_token();
    services.keyring.delete_token()?;
    let _ = services.db.clear_active_account();
    Ok(())
}

fn map_keyring_error(e: KeyringError) -> AuthError {
    AuthError::Internal(format!("keyring: {e}"))
}
