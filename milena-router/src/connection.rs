use deadpool::managed::{Manager, Object, RecycleResult};
use milena_protos::cache_server::cache_client::CacheClient;
use std::time::Duration;
use thiserror::Error;
use tonic::transport::Channel;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Failed to create connection: {0}")]
    CreateError(String),
    #[error("Failed to recycle connection: {0}")]
    RecycleError(String),
}

pub struct CacheClientManager {
    endpoint: String,
}

impl CacheClientManager {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }
}

#[async_trait::async_trait]
impl Manager for CacheClientManager {
    type Type = CacheClient<Channel>;
    type Error = ConnectionError;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        CacheClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| ConnectionError::CreateError(e.to_string()))
    }

    async fn recycle(&self, client: &mut Self::Type) -> RecycleResult<Self::Error> {
        // Check if the connection is still valid by making a health check
        // For now, we'll just assume the connection is valid if it exists
        Ok(())
    }
}

// Create a wrapper type for Object<CacheClientManager> that simplifies access to the client
pub struct PooledClient(pub Object<CacheClientManager>);

impl PooledClient {
    pub fn client(&mut self) -> &mut CacheClient<Channel> {
        &mut self.0
    }
}

pub type Pool = deadpool::managed::Pool<CacheClientManager>;

pub async fn create_pool(endpoint: String, max_size: usize) -> Result<Pool, ConnectionError> {
    let manager = CacheClientManager::new(endpoint);
    Pool::builder(manager)
        .max_size(max_size)
        .build()
        .map_err(|e| ConnectionError::CreateError(e.to_string()))
}
