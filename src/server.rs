use cache_server::{
    cache_server::{Cache, CacheServer},
    DeleteRequest, DeleteResponse, GetRequest, GetResponse, PutRequest, PutResponse,
};
use futures_core::Stream;
use std::pin::Pin;

use tonic::{transport::Server, Status};

pub mod cache_server {
    tonic::include_proto!("cache_server");
}

#[derive(Debug, Default)]
pub struct CacheService {}

#[tonic::async_trait]
impl Cache for CacheService {
    type GetStream = Pin<Box<dyn Stream<Item = Result<GetResponse, Status>> + Send>>;

    async fn get(
        &self,
        request: tonic::Request<GetRequest>,
    ) -> Result<tonic::Response<Self::GetStream>, tonic::Status> {
        todo!()
    }

    async fn put(
        &self,
        request: tonic::Request<tonic::Streaming<PutRequest>>,
    ) -> Result<tonic::Response<PutResponse>, tonic::Status> {
        todo!()
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteRequest>,
    ) -> Result<tonic::Response<DeleteResponse>, tonic::Status> {
        todo!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = CacheService::default();

    Server::builder()
        .add_service(CacheServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
