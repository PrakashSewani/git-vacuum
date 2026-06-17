pub trait KeyringStore: Send + Sync {
    fn set_token(&self, token: &str) -> Result<(), crate::error::KeyringError>;
    fn get_token(&self) -> Result<Option<String>, crate::error::KeyringError>;
    fn delete_token(&self) -> Result<(), crate::error::KeyringError>;
    fn has_token(&self) -> bool {
        self.get_token().ok().flatten().is_some()
    }
}
