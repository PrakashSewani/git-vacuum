use git_vacuum_core::{AuthError, DiscoveryError};

pub fn map_auth_error(e: octocrab::Error) -> AuthError {
    if let octocrab::Error::GitHub { source, .. } = &e {
        let status = source.status_code.as_u16();
        let msg = source.message.clone();
        match status {
            401 => return AuthError::InvalidToken,
            403 if serde_json::to_string(&source.errors)
                .map(|s| s.contains("SAML"))
                .unwrap_or(false) =>
            {
                return AuthError::SsoRequired { org: msg };
            }
            _ => return AuthError::Internal(format!("github {}: {}", status, msg)),
        }
    }
    AuthError::Internal(e.to_string())
}

pub fn map_discovery_error(e: octocrab::Error) -> DiscoveryError {
    if let octocrab::Error::GitHub { source, .. } = &e {
        let status = source.status_code.as_u16();
        let msg = source.message.clone();
        return match status {
            401 => DiscoveryError::Auth(msg),
            403 => DiscoveryError::Forbidden(msg),
            404 => DiscoveryError::NotFound(msg),
            500..=599 => DiscoveryError::ServerError {
                status,
                message: msg,
            },
            _ => DiscoveryError::Internal(format!("github {status}: {msg}")),
        };
    }
    DiscoveryError::Internal(e.to_string())
}
