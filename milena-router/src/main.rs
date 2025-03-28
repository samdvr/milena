mod connection;
mod rate_limit;
mod service;
mod validation;

use conhash::ConsistentHash;
use milena_protos::router_server::router_server::RouterServer;
use service::RouterServiceImpl;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .init();

    info!("Starting router service...");

    // Initialize rate limiter (100 requests per second)
    let rate_limiter = Arc::new(rate_limit::RateLimiterMiddleware::new(100));

    // Initialize router service
    let router_service = RouterServiceImpl {
        nodes: Arc::new(Mutex::new(ConsistentHash::new())),
        node_conns: Arc::new(Mutex::new(std::collections::HashMap::new())),
        rate_limiter,
    };

    // Setup graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));
    let shutdown_tx_clone = shutdown_tx.clone();

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl+c");
        info!("Received shutdown signal");

        if let Some(tx) = shutdown_tx_clone.lock().await.take() {
            let _ = tx.send(());
        }
    });

    // Start gRPC server
    let addr = "[::1]:50052".parse()?;
    let grpc_server = Server::builder()
        .add_service(RouterServer::new(router_service))
        .serve(addr);

    info!("Router service listening on {}", addr);

    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_rx => {
            info!("Shutting down router service...");
        }
        _ = grpc_server => {
            error!("gRPC server error");
        }
    }

    Ok(())
}
