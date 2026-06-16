use std::sync::Arc;

use git_vacuum_core::AppEvent;
use git_vacuum_core::traits::{Database, GithubApi, KeyringStore};
use tokio::sync::mpsc;

pub async fn authenticate_pat(
    token: String,
    github: Arc<dyn GithubApi>,
    db: Arc<dyn Database>,
    keyring: Arc<dyn KeyringStore>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
) {
    log::info!("Authenticating PAT token (length: {})", token.len());
    github.set_token(&token).await;
    log::info!("Token set on GitHub client, validating...");

    match github.validate_token().await {
        Ok(user) => {
            log::info!("Token validated successfully for user: {}", user.login);
            let _ = keyring.set_token("git-vacuum", "github", &token).await;
            if let Err(e) = db.upsert_account(&user).await {
                log::warn!("Failed to save account info: {}", e);
            }
            let _ = app_tx.send(AppEvent::AuthSucceeded {
                username: user.login.clone(),
                scopes: user.scopes.clone(),
                token_expires: user.token_expires_at,
            });
        }
        Err(e) => {
            log::error!("Token validation failed: {}", e);
            let _ = app_tx.send(AppEvent::AuthFailed {
                reason: "invalid_token".to_string(),
                detail: e,
            });
        }
    }
}

pub async fn device_flow_init(
    client_id: String,
    scopes: Vec<String>,
    github: Arc<dyn GithubApi>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
) -> Result<String, String> {
    let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
    let init = github.device_flow_init(&client_id, &scope_refs).await?;
    let _ = app_tx.send(AppEvent::OAuthCodeReceived {
        user_code: init.user_code.clone(),
        verification_uri: init.verification_uri.clone(),
        expires_in: init.expires_in,
    });
    Ok(init.device_code)
}

pub async fn poll_oauth_token(
    client_id: String,
    device_code: String,
    interval: std::time::Duration,
    github: Arc<dyn GithubApi>,
    _db: Arc<dyn Database>,
    keyring: Arc<dyn KeyringStore>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *cancel_rx.borrow() { break; }
        match github.device_flow_poll(&client_id, &device_code).await {
            Ok(git_vacuum_core::DeviceFlowPoll::Success { access_token, .. }) => {
                let _ = keyring.set_token("git-vacuum", "github", &access_token).await;
                let _ = app_tx.send(AppEvent::AuthSucceeded {
                    username: access_token, scopes: vec![], token_expires: None,
                });
                break;
            }
            Ok(git_vacuum_core::DeviceFlowPoll::Pending) => {
                tokio::time::sleep(interval).await;
            }
            Ok(git_vacuum_core::DeviceFlowPoll::SlowDown { new_interval }) => {
                tokio::time::sleep(new_interval).await;
            }
            Ok(git_vacuum_core::DeviceFlowPoll::Expired) => {
                let _ = app_tx.send(AppEvent::OAuthTimeout);
                break;
            }
            Ok(git_vacuum_core::DeviceFlowPoll::AccessDenied) => {
                let _ = app_tx.send(AppEvent::AuthFailed {
                    reason: "access_denied".to_string(),
                    detail: "User denied access".to_string(),
                });
                break;
            }
            Err(e) => {
                let _ = app_tx.send(AppEvent::AuthFailed {
                    reason: "oauth_error".to_string(),
                    detail: e,
                });
                break;
            }
        }
    }
}
