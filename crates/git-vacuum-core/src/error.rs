#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Rate limited: {0}")]
    RateLimit(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Disk full: {0}")]
    DiskFull(String),

    #[error("Repository corrupt: {0}")]
    RepoCorrupt(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Internal error: {0}")]
    Internal(String),
}
