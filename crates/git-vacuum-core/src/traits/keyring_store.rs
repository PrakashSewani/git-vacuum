use async_trait::async_trait;

#[async_trait]
pub trait KeyringStore: Send + Sync {
    async fn set_token(&self, service: &str, username: &str, token: &str) -> Result<(), String>;
    async fn get_token(&self, service: &str, username: &str) -> Result<Option<String>, String>;
    async fn delete_token(&self, service: &str, username: &str) -> Result<(), String>;
}
