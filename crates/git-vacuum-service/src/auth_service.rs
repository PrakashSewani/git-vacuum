use std::sync::Arc;
use std::time::Duration;

use git_vacuum_core::{AuthError, DeviceFlowInit, DeviceFlowPoll, KeyringError, UserInfo};

use crate::Services;

/// Validate a PAT against GitHub, then persist it to the OS keyring
/// and upsert the account metadata in the database.
/// Token goes to keyring ONLY (never to SQLite, never logged).
pub async fn authenticate_pat(services: Arc<Services>, token: &str) -> Result<UserInfo, AuthError> {
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

/// GitHub OAuth App client_id is required for the device authorization flow.
/// The user must register their own OAuth App at:
///   https://github.com/settings/applications/new
/// and pass the client_id via the `--oauth-client-id` flag or
/// `GIT_VACUUM_OAUTH_CLIENT_ID` env var. We intentionally do not embed a
/// default client_id because:
/// 1. GitHub's OAuth policy requires each app to identify itself.
/// 2. A shared client_id would let any user poll on behalf of any other.
/// 3. Registration takes < 2 minutes and is free.
pub async fn start_oauth_device_flow(
    services: Arc<Services>,
    client_id: &str,
) -> Result<DeviceFlowInit, AuthError> {
    if client_id.is_empty() {
        return Err(AuthError::Internal(
            "OAuth client_id is required. Pass --oauth-client-id <id> or set GIT_VACUUM_OAUTH_CLIENT_ID. Register at https://github.com/settings/applications/new".into()
        ));
    }
    services
        .github
        .device_flow_init(client_id, &["repo", "read:org", "user"])
        .await
}

pub async fn poll_oauth_device_flow(
    services: Arc<Services>,
    client_id: &str,
    device_code: String,
) -> Result<DeviceFlowPoll, AuthError> {
    if client_id.is_empty() {
        return Err(AuthError::Internal("OAuth client_id missing".into()));
    }
    services
        .github
        .device_flow_poll(client_id, &device_code)
        .await
}

pub async fn complete_oauth_with_token(
    services: Arc<Services>,
    token: String,
) -> Result<UserInfo, AuthError> {
    if token.is_empty() {
        return Err(AuthError::InvalidToken);
    }
    services.github.set_token(&token);
    let info = services.github.validate_token().await?;
    services
        .keyring
        .set_token(&token)
        .map_err(map_keyring_error)?;
    services
        .db
        .upsert_account(&info)
        .map_err(|e| AuthError::Internal(format!("db upsert: {e}")))?;
    Ok(info)
}

/// Default polling interval (5s, matches GitHub's recommended device-flow cadence)
pub fn default_poll_interval() -> Duration {
    Duration::from_secs(5)
}

fn map_keyring_error(e: KeyringError) -> AuthError {
    AuthError::Internal(format!("keyring: {e}"))
}
