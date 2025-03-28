# Milena Protocol Definitions

This package contains the Protocol Buffer definitions for the Milena distributed caching system. These definitions specify the gRPC service interfaces and message formats used for communication between system components and clients.

## Services

### Cache Service

The `CacheServer` service defines the API for individual cache nodes:

```protobuf
service Cache {
  rpc Get(GetRequest) returns (GetResponse);
  rpc Put(PutRequest) returns (PutResponse);
  rpc Delete(DeleteRequest) returns (DeleteResponse);
}
```

### Router Service

The `Router` service defines the API for the router component, which clients interact with:

```protobuf
service Router {
  // Data operations
  rpc Get(GetRequest) returns (GetResponse);
  rpc Put(PutRequest) returns (PutResponse);
  rpc Delete(DeleteRequest) returns (DeleteResponse);

  // Node management
  rpc Join(JoinRequest) returns (JoinResponse);
  rpc Leave(LeaveRequest) returns (LeaveResponse);
}
```

## Message Types

### Request Messages

- **GetRequest**: Request to retrieve a value

  ```protobuf
  message GetRequest {
    bytes key = 1;
    string bucket = 2;
  }
  ```

- **PutRequest**: Request to store a value

  ```protobuf
  message PutRequest {
    bytes key = 1;
    string bucket = 2;
    bytes value = 3;
  }
  ```

- **DeleteRequest**: Request to delete a value

  ```protobuf
  message DeleteRequest {
    bytes key = 1;
    string bucket = 2;
  }
  ```

- **JoinRequest**: Request for a cache node to join the cluster

  ```protobuf
  message JoinRequest {
    string address = 1;
  }
  ```

- **LeaveRequest**: Request for a cache node to leave the cluster
  ```protobuf
  message LeaveRequest {
    string address = 1;
  }
  ```

### Response Messages

- **GetResponse**: Response containing the requested value

  ```protobuf
  message GetResponse {
    bool successful = 1;
    bytes value = 2;
  }
  ```

- **PutResponse**: Response indicating the success of a put operation

  ```protobuf
  message PutResponse {
    bool successful = 1;
  }
  ```

- **DeleteResponse**: Response indicating the success of a delete operation

  ```protobuf
  message DeleteResponse {
    bool successful = 1;
  }
  ```

- **JoinResponse**: Response indicating the success of a join operation

  ```protobuf
  message JoinResponse {
    bool successful = 1;
  }
  ```

- **LeaveResponse**: Response indicating the success of a leave operation
  ```protobuf
  message LeaveResponse {
    bool successful = 1;
  }
  ```

## Code Generation

This package uses `tonic-build` to generate Rust code from the Protocol Buffer definitions at build time. The build process is defined in `build.rs`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/cache_server.proto")?;
    tonic_build::compile_protos("proto/router_server.proto")?;
    Ok(())
}
```

The generated code includes:

- Rust structs for all message types
- Client implementations for calling the services
- Trait definitions for implementing the services
- Serialization/deserialization code

## Using the Generated Code

### For Clients

To interact with the Milena cache as a client:

```rust
use milena_protos::router_server::router_client::RouterClient;
use milena_protos::router_server::{GetRequest, PutRequest};
use tonic::Request;

async fn client_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RouterClient::connect("http://localhost:50050").await?;

    // Store a value
    let put_req = Request::new(PutRequest {
        key: b"example_key".to_vec(),
        bucket: "default".to_string(),
        value: b"example_value".to_vec(),
    });
    let _put_response = client.put(put_req).await?;

    // Retrieve a value
    let get_req = Request::new(GetRequest {
        key: b"example_key".to_vec(),
        bucket: "default".to_string(),
    });
    let get_response = client.get(get_req).await?;

    Ok(())
}
```

### For Implementing Services

To implement the cache service:

```rust
use milena_protos::cache_server::{
    cache_server::Cache, GetRequest, GetResponse, PutRequest, PutResponse,
    DeleteRequest, DeleteResponse,
};
use tonic::{Request, Response, Status};

pub struct MyCacheService;

#[tonic::async_trait]
impl Cache for MyCacheService {
    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<GetResponse>, Status> {
        // Implementation here
    }

    async fn put(
        &self,
        request: Request<PutRequest>,
    ) -> Result<Response<PutResponse>, Status> {
        // Implementation here
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        // Implementation here
    }
}
```

## Development

### Modifying Protocol Definitions

To modify the API:

1. Edit the Protocol Buffer files in the `proto/` directory
2. Run `cargo build` to regenerate the Rust code
3. Update implementations in the other crates as needed

### Dependencies

- `prost`: Protocol Buffers implementation for Rust
- `tonic`: gRPC implementation for Rust
- `tonic-build`: Code generator for Protocol Buffers and gRPC
