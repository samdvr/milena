use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub aws_region: String,
    pub lru_size: usize,
    pub ttl_seconds: u64,
    pub router_addr: String,
    pub s3_bucket: String,
    pub log_level: String,
    pub metrics_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::Environment::default())
            .build()
            .map_err(|e| ConfigError::InvalidConfig(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| ConfigError::InvalidConfig(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.lru_size == 0 {
            return Err(ConfigError::InvalidConfig(
                "LRU size must be greater than 0".to_string(),
            ));
        }
        if self.ttl_seconds == 0 {
            return Err(ConfigError::InvalidConfig(
                "TTL must be greater than 0".to_string(),
            ));
        }
        if self.router_addr.is_empty() {
            return Err(ConfigError::MissingConfig(
                "Router address is required".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "[::1]:50051".parse().unwrap(),
            aws_region: "us-east-1".to_string(),
            lru_size: 100,
            ttl_seconds: 360,
            router_addr: "http://localhost:50052".to_string(),
            s3_bucket: "milena-cache".to_string(),
            log_level: "info".to_string(),
            metrics_port: 9090,
        }
    }
}
