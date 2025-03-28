mod config;
mod error;
mod metrics;
mod operation;
mod service;
mod store;

use crate::config::Config;
use crate::metrics::Metrics;
use crate::operation::Operation;
use crate::service::CacheService;
use crate::store::{DiskStore, LRUStore, S3Store};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use aws_types::region::Region;
use cache_server::cache_server::CacheServer;
use milena_protos::cache_server;
use milena_protos::router_server::router_client::RouterClient;
use prometheus::Encoder;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tonic::transport::Server;
use tracing::{error, info, warn};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration
    let config = Config::from_env()?;
    config.validate()?;

    // Initialize logging
    tracing_subscriber::fmt().init();

    // Initialize metrics
    let metrics = Metrics::new()?;
    let metrics_clone = metrics.clone();

    // Initialize AWS S3 client
    let region_provider =
        RegionProviderChain::default_provider().or_else(Region::new(config.aws_region.clone()));
    let aws_config = aws_config::from_env().region(region_provider).load().await;
    let s3_client = Client::new(&aws_config);

    // Initialize cache service
    let service = CacheService {
        operation: Arc::new(Mutex::new(
            Operation::<LRUStore, DiskStore, S3Store>::simple_new(
                config.lru_size as u64,
                Duration::from_secs(config.ttl_seconds),
                s3_client,
            ),
        )),
        metrics: Arc::new(metrics),
    };

    // Setup graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let shutdown_tx_clone = shutdown_tx;

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl+c");
        info!("Received shutdown signal");
        shutdown_tx_clone
            .send(())
            .expect("Failed to send shutdown signal");
    });

    // Start metrics server
    let metrics_addr =
        format!("0.0.0.0:{}", config.metrics_port).parse::<std::net::SocketAddr>()?;
    let metrics_server = warp::serve(warp::path("metrics").boxed().and(warp::get().boxed()).map(
        move || {
            let mut buffer = Vec::new();
            prometheus::TextEncoder::new()
                .encode(&metrics_clone.registry.gather(), &mut buffer)
                .unwrap();
            warp::reply::with_header(
                buffer,
                "Content-Type",
                "text/plain; version=0.0.4; charset=utf-8",
            )
        },
    ))
    .run(metrics_addr);

    // Start gRPC server
    let grpc_server = Server::builder()
        .add_service(CacheServer::new(service))
        .serve(config.listen_addr);

    // Join router
    let mut router_client =
        RouterClient::connect(config.router_addr.parse::<tonic::transport::Uri>()?).await?;
    if let Err(e) = router_client
        .join(milena_protos::router_server::JoinRequest {
            address: config.listen_addr.to_string(),
        })
        .await
    {
        warn!("Failed to join router: {}", e);
    }

    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_rx => {
            info!("Shutting down...");
        }
        _ = grpc_server => {
            error!("gRPC server error");
        }
        _ = metrics_server => {
            error!("Metrics server error");
        }
    }

    Ok(())
}
