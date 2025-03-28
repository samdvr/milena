use crate::{
    connection::{CacheClientManager, Pool, PooledClient},
    rate_limit::{RateLimitError, RateLimiterMiddleware},
    validation::{
        validate_address, validate_bucket_name, validate_key, validate_value, ValidationError,
    },
};
use conhash::{ConsistentHash, Node};
use milena_protos::cache_server::{self};
use milena_protos::router_server::{router_server::Router, *};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Validation error: {0}")]
    ValidationError(#[from] ValidationError),
    #[error("Rate limit error: {0}")]
    RateLimitError(#[from] RateLimitError),
}

// Define a helper type for our result to avoid confusion with Status
pub type RouterResult<T> = std::result::Result<T, RouterError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServerNode {
    host: String,
}

impl Node for ServerNode {
    fn name(&self) -> String {
        self.host.to_string()
    }
}

pub struct RouterServiceImpl {
    pub nodes: Arc<Mutex<ConsistentHash<ServerNode>>>,
    pub node_conns: Arc<Mutex<HashMap<String, Pool>>>,
    pub rate_limiter: Arc<RateLimiterMiddleware>,
}

impl RouterServiceImpl {
    async fn get_connection_for_key(&self, key: &Vec<u8>) -> RouterResult<PooledClient> {
        let nodes_guard = self.nodes.lock().await;
        let node = nodes_guard.get(key).ok_or_else(|| {
            RouterError::NodeNotFound(format!("No node found for key: {:?}", key))
        })?;

        let node_conns_guard = self.node_conns.lock().await;
        let pool = node_conns_guard.get(&node.host).ok_or_else(|| {
            RouterError::NodeNotFound(format!("No connection found for node: {}", node.host))
        })?;

        // Get connection from pool
        let connection = pool
            .get()
            .await
            .map_err(|e| RouterError::ConnectionError(e.to_string()))?;
        Ok(PooledClient(connection))
    }

    async fn join_node(&self, address: String) -> RouterResult<()> {
        info!("Joining node: {}", address);
        validate_address(&address)?;

        self.nodes.lock().await.add(
            &ServerNode {
                host: address.clone(),
            },
            2,
        );

        // Create a connection pool for the new node
        let pool = Pool::builder(CacheClientManager::new(address.clone()))
            .max_size(10)
            .build()
            .map_err(|e| RouterError::ConnectionError(e.to_string()))?;

        self.node_conns.lock().await.insert(address, pool);
        info!("Successfully joined node");
        Ok(())
    }

    async fn leave_node(&self, address: String) {
        info!("Leaving node: {}", address);
        self.nodes.lock().await.remove(&ServerNode {
            host: address.clone(),
        });
        self.node_conns.lock().await.remove(&address);
        info!("Successfully removed node");
    }
}

#[tonic::async_trait]
impl Router for RouterServiceImpl {
    async fn join(
        &self,
        request: tonic::Request<JoinRequest>,
    ) -> std::result::Result<Response<JoinResponse>, Status> {
        match self.rate_limiter.check_rate_limit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(
                    Code::ResourceExhausted,
                    format!("Rate limit exceeded: {}", e),
                ));
            }
        }

        let request_ref = request.into_inner();
        match self.join_node(request_ref.address).await {
            Ok(_) => Ok(Response::new(JoinResponse { successful: true })),
            Err(e) => {
                error!("Failed to join node: {}", e);
                Err(Status::new(Code::Internal, format!("{e}")))
            }
        }
    }

    async fn leave(
        &self,
        request: tonic::Request<LeaveRequest>,
    ) -> std::result::Result<Response<LeaveResponse>, Status> {
        match self.rate_limiter.check_rate_limit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(
                    Code::ResourceExhausted,
                    format!("Rate limit exceeded: {}", e),
                ));
            }
        }

        let request_ref = request.into_inner();
        self.leave_node(request_ref.address).await;
        Ok(Response::new(LeaveResponse { successful: true }))
    }

    async fn get(
        &self,
        request: tonic::Request<GetRequest>,
    ) -> std::result::Result<Response<GetResponse>, Status> {
        match self.rate_limiter.check_rate_limit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(
                    Code::ResourceExhausted,
                    format!("Rate limit exceeded: {}", e),
                ));
            }
        }

        let request_ref = request.into_inner();
        match validate_bucket_name(&request_ref.bucket) {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
            }
        }

        match validate_key(&request_ref.key) {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
            }
        }

        match self.get_connection_for_key(&request_ref.key).await {
            Ok(mut pooled_client) => {
                match pooled_client
                    .client()
                    .get(Request::new(cache_server::GetRequest {
                        key: request_ref.key,
                        bucket: request_ref.bucket,
                    }))
                    .await
                {
                    Ok(x) => {
                        let response = x.into_inner();
                        Ok(Response::new(GetResponse {
                            value: response.value,
                            successful: response.successful,
                        }))
                    }
                    Err(e) => {
                        error!("Failed to get key: {}", e);
                        Err(Status::new(Code::Internal, format!("{e}")))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get connection: {}", e);
                Err(Status::new(Code::Internal, format!("{e}")))
            }
        }
    }

    async fn put(
        &self,
        request: tonic::Request<PutRequest>,
    ) -> std::result::Result<Response<PutResponse>, Status> {
        match self.rate_limiter.check_rate_limit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(
                    Code::ResourceExhausted,
                    format!("Rate limit exceeded: {}", e),
                ));
            }
        }

        let request_ref = request.into_inner();
        if let Err(e) = validate_bucket_name(&request_ref.bucket) {
            return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
        }
        if let Err(e) = validate_key(&request_ref.key) {
            return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
        }
        if let Err(e) = validate_value(&request_ref.value) {
            return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
        }

        match self.get_connection_for_key(&request_ref.key).await {
            Ok(mut pooled_client) => {
                match pooled_client
                    .client()
                    .put(Request::new(cache_server::PutRequest {
                        key: request_ref.key,
                        bucket: request_ref.bucket,
                        value: request_ref.value,
                    }))
                    .await
                {
                    Ok(x) => {
                        let response = x.into_inner();
                        Ok(Response::new(PutResponse {
                            successful: response.successful,
                        }))
                    }
                    Err(e) => {
                        error!("Failed to put key: {}", e);
                        Err(Status::new(Code::Internal, format!("{e}")))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get connection: {}", e);
                Err(Status::new(Code::Internal, format!("{e}")))
            }
        }
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteRequest>,
    ) -> std::result::Result<Response<DeleteResponse>, Status> {
        match self.rate_limiter.check_rate_limit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(Status::new(
                    Code::ResourceExhausted,
                    format!("Rate limit exceeded: {}", e),
                ));
            }
        }

        let request_ref = request.into_inner();
        if let Err(e) = validate_bucket_name(&request_ref.bucket) {
            return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
        }
        if let Err(e) = validate_key(&request_ref.key) {
            return Err(Status::new(Code::InvalidArgument, format!("{}", e)));
        }

        match self.get_connection_for_key(&request_ref.key).await {
            Ok(mut pooled_client) => {
                match pooled_client
                    .client()
                    .delete(Request::new(cache_server::DeleteRequest {
                        key: request_ref.key,
                        bucket: request_ref.bucket,
                    }))
                    .await
                {
                    Ok(x) => {
                        let response = x.into_inner();
                        Ok(Response::new(DeleteResponse {
                            successful: response.successful,
                        }))
                    }
                    Err(e) => {
                        error!("Failed to delete key: {}", e);
                        Err(Status::new(Code::Internal, format!("{e}")))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get connection: {}", e);
                Err(Status::new(Code::Internal, format!("{e}")))
            }
        }
    }
}
