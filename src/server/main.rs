use crate::{
    operation::Operation,
    store::{DiskStore, Key, LRUStore, S3Store, Value},
};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use cache_server::{
    cache_server::{Cache, CacheServer},
    DeleteRequest, DeleteResponse, GetRequest, GetResponse, PutRequest, PutResponse,
};

use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tonic::{transport::Server, Code, Response, Status};

pub mod cache_server {
    tonic::include_proto!("cache_server");
}

pub struct CacheService {
    operation: Arc<Mutex<Operation<LRUStore, DiskStore, S3Store>>>,
}

#[tonic::async_trait]
impl Cache for CacheService {
    async fn get(
        &self,
        request: tonic::Request<GetRequest>,
    ) -> Result<tonic::Response<GetResponse>, tonic::Status> {
        let request_ref = request.get_ref();
        let key = Key(request_ref.key.clone());
        let bucket = &request_ref.bucket;
        let result = self.operation.lock().await.get(bucket, &key).await;

        match result {
            Ok(Some(data)) => {
                let value = data.0;
                let successful = true;

                Ok(Response::new(GetResponse { value, successful }))
            }
            Ok(None) => Err(Status::new(Code::NotFound, "not_found")),
            Err(e) => Err(Status::new(Code::Internal, e)),
        }
    }

    async fn put(
        &self,
        request: tonic::Request<PutRequest>,
    ) -> Result<tonic::Response<PutResponse>, tonic::Status> {
        let request_ref = request.get_ref();
        let key = Key(request_ref.key.clone());
        let bucket = &request_ref.bucket;
        let value = request_ref.clone().value;
        let result = self
            .operation
            .lock()
            .await
            .put(bucket, &key, &Value(value))
            .await;

        match result {
            Ok(()) => Ok(Response::new(PutResponse { successful: true })),
            Err(e) => Err(Status::new(Code::Internal, e)),
        }
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteRequest>,
    ) -> Result<tonic::Response<DeleteResponse>, tonic::Status> {
        let request_ref = request.get_ref();
        let bucket = &request_ref.bucket;
        let result = self
            .operation
            .lock()
            .await
            .delete(bucket, &Key(vec![1]))
            .await;

        match result {
            Ok(()) => Ok(Response::new(DeleteResponse { successful: true })),
            Err(e) => Err(Status::new(Code::Internal, e)),
        }
    }
}

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
                Duration::from_secs(100000),
                client,
            ),
        )),
    };

    Server::builder()
        .add_service(CacheServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
