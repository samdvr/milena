mod operation;
mod service;
mod store;

use crate::operation::Operation;
use crate::service::CacheService;
use crate::store::DiskStore;
use crate::store::LRUStore;
use crate::store::S3Store;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use cache_server::cache_server::CacheServer;
use milena_protos::cache_server;
use milena_protos::router_server::router_client::RouterClient;
use milena_protos::router_server::JoinRequest;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let client = Client::new(&config);
    let service = CacheService {
        operation: Arc::new(Mutex::new(
            Operation::<LRUStore, DiskStore, S3Store>::simple_new(
                100,
                Duration::from_secs(360),
                client,
            ),
        )),
    };
    RouterClient::connect("dst")
        .await
        .unwrap()
        .join(tonic::Request::new(JoinRequest {
            address: "".to_string(),
        }))
        .await?;

    Server::builder()
        .add_service(CacheServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
