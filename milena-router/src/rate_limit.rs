use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

pub struct RateLimiterMiddleware {
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl RateLimiterMiddleware {
    pub fn new(requests_per_second: u32) -> Self {
        let limiter = RateLimiter::direct(Quota::per_second(
            NonZeroU32::new(requests_per_second).unwrap(),
        ));
        Self {
            limiter: Arc::new(limiter),
        }
    }

    pub async fn check_rate_limit(&self) -> Result<(), RateLimitError> {
        self.limiter
            .check()
            .map_err(|_| RateLimitError::RateLimitExceeded)
    }
}
