use crate::{
    error::Result,
    metrics::Metrics,
    operation::Operation,
    store::{DiskStore, Key, LRUStore, S3Store, Value},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Response, Status};

use milena_protos::cache_server::{
    cache_server::Cache, DeleteRequest, DeleteResponse, GetRequest, GetResponse, PutRequest,
    PutResponse,
};

pub struct CacheService {
    pub operation: Arc<Mutex<Operation<LRUStore, DiskStore, S3Store>>>,
    pub metrics: Arc<Metrics>,
}

#[tonic::async_trait]
impl Cache for CacheService {
    async fn get(
        &self,
        request: tonic::Request<GetRequest>,
    ) -> std::result::Result<Response<GetResponse>, tonic::Status> {
        let timer = self.metrics.operation_duration.start_timer();
        self.metrics.request_counter.inc();

        let request_ref = request.into_inner();
        let key = Key(request_ref.key);
        let bucket = &request_ref.bucket;

        let result = self
            .operation
            .lock()
            .await
            .get(bucket, &key)
            .await
            .map_err(|e| {
                self.metrics.error_counter.inc();
                tonic::Status::new(tonic::Code::Internal, format!("{e}"))
            })?;
        timer.observe_duration();

        if let Some(v) = result {
            self.metrics.cache_hits.inc();
            Ok(Response::new(GetResponse {
                successful: true,
                value: v.0,
            }))
        } else {
            self.metrics.cache_misses.inc();
            Ok(Response::new(GetResponse {
                successful: true,
                value: vec![],
            }))
        }
    }

    async fn put(
        &self,
        request: tonic::Request<milena_protos::cache_server::PutRequest>,
    ) -> std::result::Result<Response<PutResponse>, tonic::Status> {
        let timer = self.metrics.operation_duration.start_timer();
        self.metrics.request_counter.inc();

        let request_ref = request.into_inner();
        let key = Key(request_ref.key);
        let bucket = &request_ref.bucket;
        let value = request_ref.value;

        self.operation
            .lock()
            .await
            .put(bucket, &key, &Value(value))
            .await
            .map_err(|e| {
                self.metrics.error_counter.inc();
                tonic::Status::new(tonic::Code::Internal, format!("{e}"))
            })?;
        timer.observe_duration();

        Ok(Response::new(PutResponse { successful: true }))
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteRequest>,
    ) -> std::result::Result<Response<DeleteResponse>, tonic::Status> {
        let timer = self.metrics.operation_duration.start_timer();
        self.metrics.request_counter.inc();

        let request_ref = request.into_inner();
        let key = request_ref.key;
        let bucket = &request_ref.bucket;

        self.operation
            .lock()
            .await
            .delete(bucket, &Key(key))
            .await
            .map_err(|e| {
                self.metrics.error_counter.inc();
                tonic::Status::new(tonic::Code::Internal, format!("{e}"))
            })?;
        timer.observe_duration();

        Ok(Response::new(DeleteResponse { successful: true }))
    }
}
