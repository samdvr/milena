use conhash::{ConsistentHash, Node};
use milena_protos::cache_server::cache_client::CacheClient;
use milena_protos::cache_server::{self, cache_client};
use milena_protos::router_server::{router_server::Router, *};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use tonic::{Code, Response, Status};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServerNode {
    host: String,
}

impl Node for ServerNode {
    fn name(&self) -> String {
        format!("{}", self.host)
    }
}

pub struct RouterServiceImpl {
    pub nodes: Arc<Mutex<ConsistentHash<ServerNode>>>,
    pub node_conns: Arc<Mutex<HashMap<String, CacheClient<Channel>>>>,
}
impl RouterServiceImpl {
    async fn get_connection_for_key(
        &self,
        key: &Vec<u8>,
    ) -> Result<CacheClient<Channel>, tonic::transport::Error> {
        // Get the node address using consistent hashing
        let nodes_guard = self.nodes.lock().await;
        let node_address = &nodes_guard.get(&key).unwrap().host;
        let connection = self
            .node_conns
            .lock()
            .await
            .get(node_address)
            .unwrap()
            .clone();
        Ok(connection)
    }

    async fn join_node(&self, address: String) -> Result<(), tonic::transport::Error> {
        self.nodes.lock().await.add(
            &ServerNode {
                host: address.clone(),
            },
            2,
        );

        // Connect to the new node and store the connection
        let conn = cache_client::CacheClient::connect(address.clone()).await?;
        self.node_conns.lock().await.insert(address, conn);
        Ok(())
    }

    async fn leave_node(&self, address: String) {
        self.nodes.lock().await.remove(&ServerNode {
            host: address.clone(),
        });
        self.node_conns.lock().await.remove(&address.clone());
    }
}

#[tonic::async_trait]
impl Router for RouterServiceImpl {
    async fn join(
        &self,
        request: tonic::Request<JoinRequest>,
    ) -> Result<Response<JoinResponse>, Status> {
        let request_ref = request.into_inner();
        match self.join_node(request_ref.address).await {
            Ok(_) => Ok(Response::new(JoinResponse { successful: true })),
            Err(e) => Err(Status::new(Code::Internal, format!("{e}"))),
        }
    }

    async fn leave(
        &self,
        request: tonic::Request<LeaveRequest>,
    ) -> Result<Response<LeaveResponse>, Status> {
        let request_ref = request.into_inner();
        self.leave_node(request_ref.address).await;

        Ok(Response::new(LeaveResponse { successful: true }))
    }

    async fn get(
        &self,
        request: tonic::Request<GetRequest>,
    ) -> Result<Response<GetResponse>, Status> {
        let request_ref = request.into_inner();
        let key = request_ref.key;
        let mut connection = self.get_connection_for_key(&key).await.unwrap();
        connection
            .get(tonic::Request::new(cache_server::GetRequest {
                key,
                bucket: request_ref.bucket,
            }))
            .await
            .map(|x| {
                let response = x.into_inner();
                Response::new(GetResponse {
                    value: response.value,
                    successful: response.successful,
                })
            })
    }

    async fn put(
        &self,
        request: tonic::Request<PutRequest>,
    ) -> Result<Response<PutResponse>, Status> {
        let request_ref = request.into_inner();
        let key = request_ref.key;
        let mut connection = self.get_connection_for_key(&key).await.unwrap();
        connection
            .put(tonic::Request::new(cache_server::PutRequest {
                key,
                bucket: request_ref.bucket,
                value: request_ref.value,
            }))
            .await
            .map(|x| {
                let response = x.into_inner();
                Response::new(PutResponse {
                    successful: response.successful,
                })
            })
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let request_ref = request.into_inner();
        let key = request_ref.key;
        let mut connection = self.get_connection_for_key(&key).await.unwrap();
        connection
            .delete(tonic::Request::new(cache_server::DeleteRequest {
                key,
                bucket: request_ref.bucket,
            }))
            .await
            .map(|x| {
                let response = x.into_inner();
                Response::new(DeleteResponse {
                    successful: response.successful,
                })
            })
    }
}
