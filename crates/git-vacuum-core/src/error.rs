use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("discovery error: {0}")]
    Discovery(#[from] DiscoveryError),
    #[error("sync error: {0}")]
    Sync(#[from] SyncError),
    #[error("database error: {0}")]
    Db(#[from] DbError),
    #[error("git error: {0}")]
    Git(#[from] GitError),
    #[error("keyring error: {0}")]
    Keyring(#[from] KeyringError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid token")]
    InvalidToken,
    #[error("expired token")]
    ExpiredToken,
    #[error("insufficient scopes: required {required:?}, actual {actual:?}")]
    InsufficientScopes { required: Vec<String>, actual: Vec<String> },
    #[error("token revoked")]
    TokenRevoked,
    #[error("account suspended")]
    AccountSuspended,
    #[error("sso required for org {org}")]
    SsoRequired { org: String },
    #[error("device flow not enabled")]
    DeviceFlowDisabled,
    #[error("network: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("network: {0}")]
    Network(String),
    #[error("auth: {0}")]
    Auth(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("server error: status {status}: {message}")]
    ServerError { status: u16, message: String },
    #[error("parse: {0}")]
    Parse(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("network: {0}")]
    Network(String),
    #[error("git: {0}")]
    Git(GitError),
    #[error("auth: {0}")]
    Auth(String),
    #[error("disk full")]
    DiskFull,
    #[error("permissions: {0}")]
    Permissions(String),
    #[error("timeout")]
    Timeout,
    #[error("repository not found")]
    RepoNotFound,
    #[error("remote refused: {0}")]
    RemoteRefused(String),
    #[error("object corrupt")]
    ObjectCorrupt,
    #[error("internal: {0}")]
    Internal(String),
    #[error("filesystem: {0}")]
    Filesystem(String),
}

impl From<GitError> for SyncError {
    fn from(e: GitError) -> Self {
        SyncError::Git(e)
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git2: {0}")]
    Git2(String),
    #[error("repository not found")]
    NotFound,
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("not a git repository")]
    NotARepository,
    #[error("authentication required")]
    AuthRequired,
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(String),
    #[error("migration: {0}")]
    Migration(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum KeyringError {
    #[error("no platform keyring available. Install gnome-keyring or kwallet, or use --token <pat> for this session.")]
    NoBackend,
    #[error("entry not found")]
    NoEntry,
    #[error("platform error: {0}")]
    Platform(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required argument: {0}")]
    MissingArgument(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("internal: {0}")]
    Internal(String),
}
