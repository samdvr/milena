use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Router error: {0}")]
    RouterError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

impl From<tonic::transport::Error> for CacheError {
    fn from(err: tonic::transport::Error) -> Self {
        CacheError::ConnectionError(err.to_string())
    }
}

impl From<aws_sdk_s3::Error> for CacheError {
    fn from(err: aws_sdk_s3::Error) -> Self {
        CacheError::StorageError(err.to_string())
    }
}

impl From<std::io::Error> for CacheError {
    fn from(err: std::io::Error) -> Self {
        CacheError::StorageError(err.to_string())
    }
}
