use crate::{
    operation::Operation,
    store::{DiskStore, Key, LRUStore, S3Store, Value},
};

use cache_server::{
    cache_server::Cache, DeleteRequest, DeleteResponse, GetRequest, GetResponse, PutRequest,
    PutResponse,
};

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Response, Status};

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
        let key = Key(request_ref.clone().key);

        let bucket = &request_ref.bucket;
        let result = self.operation.lock().await.get(bucket, &key).await;

        match result {
            Ok(Some(data)) => {
                let value = data.0;
                let successful = true;

                Ok(Response::new(GetResponse { value, successful }))
            }
            Ok(None) => Err(Status::new(Code::NotFound, "not_found")),
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
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
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
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
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
        }
    }
}
