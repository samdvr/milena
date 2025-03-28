use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid bucket name: {0}")]
    InvalidBucketName(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
}

pub fn validate_bucket_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::InvalidBucketName(
            "Bucket name cannot be empty".to_string(),
        ));
    }
    if name.len() > 63 {
        return Err(ValidationError::InvalidBucketName(
            "Bucket name cannot be longer than 63 characters".to_string(),
        ));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ValidationError::InvalidBucketName(
            "Bucket name can only contain alphanumeric characters and hyphens".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_key(key: &[u8]) -> Result<(), ValidationError> {
    if key.is_empty() {
        return Err(ValidationError::InvalidKey(
            "Key cannot be empty".to_string(),
        ));
    }
    if key.len() > 1024 {
        return Err(ValidationError::InvalidKey(
            "Key cannot be longer than 1024 bytes".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_value(value: &[u8]) -> Result<(), ValidationError> {
    if value.len() > 5 * 1024 * 1024 {
        return Err(ValidationError::InvalidValue(
            "Value cannot be larger than 5MB".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_address(addr: &str) -> Result<(), ValidationError> {
    if addr.is_empty() {
        return Err(ValidationError::InvalidAddress(
            "Address cannot be empty".to_string(),
        ));
    }
    if !addr.starts_with("http://") && !addr.starts_with("https://") {
        return Err(ValidationError::InvalidAddress(
            "Address must start with http:// or https://".to_string(),
        ));
    }
    Ok(())
}
