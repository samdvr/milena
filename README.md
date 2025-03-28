# Milena - Distributed Caching System

Milena is a high-performance distributed caching system written in Rust, designed for scalability and reliability. It provides a multi-tiered caching solution with consistent hashing for request routing across a cluster of cache nodes.

## Architecture

Milena consists of three main components:

1. **Cache Server (milena-cache)**: Individual cache nodes that store and retrieve data in a multi-tiered architecture:

   - LRU in-memory cache for fast access
   - Disk-based storage for persistence
   - S3 storage for backup and overflow

2. **Router (milena-router)**: Handles client requests and routes them to the appropriate cache node using consistent hashing, ensuring even distribution and minimizing cache misses during node addition/removal.

3. **Protocol Definitions (milena-protos)**: Protocol Buffer definitions for the gRPC interfaces used by the cache and router components.

### System Design

```
                           ┌──────────────┐
                           │   Client     │
                           └───────┬──────┘
                                   │
                                   ▼
                           ┌──────────────┐
                           │    Router    │◄───► Consistent hashing
                           └───────┬──────┘     for key distribution
                                   │
                     ┌─────────────┼─────────────┐
                     │             │             │
                     ▼             ▼             ▼
              ┌─────────────┐┌─────────────┐┌─────────────┐
              │ Cache Node  ││ Cache Node  ││ Cache Node  │
              └──────┬──────┘└──────┬──────┘└──────┬──────┘
                     │             │             │
                     ▼             ▼             ▼
              ┌─────────────┐┌─────────────┐┌─────────────┐
              │ LRU Memory  ││ LRU Memory  ││ LRU Memory  │
              │    Cache    ││    Cache    ││    Cache    │
              └──────┬──────┘└──────┬──────┘└──────┬──────┘
                     │             │             │
                     ▼             ▼             ▼
              ┌─────────────┐┌─────────────┐┌─────────────┐
              │ Disk Store  ││ Disk Store  ││ Disk Store  │
              └──────┬──────┘└──────┬──────┘└──────┬──────┘
                     │             │             │
                     └─────────────┼─────────────┘
                                   │
                                   ▼
                           ┌──────────────┐
                           │  S3 Storage  │
                           └──────────────┘
```

## Features

- **Multi-tiered Caching**: Balance between speed and durability with in-memory, disk, and S3 storage.
- **Distributed Design**: Scale horizontally by adding more cache nodes.
- **Consistent Hashing**: Minimize cache redistribution when nodes join or leave.
- **Dynamic Membership**: Nodes can join or leave the cluster at runtime.
- **Rate Limiting**: Prevent API abuse with configurable rate limiting.
- **Monitoring**: Prometheus metrics for monitoring cache performance and health.
- **gRPC Interface**: Fast, efficient communication between components.

## Getting Started

### Prerequisites

- Rust 1.65 or higher
- Docker (optional, for containerized deployment)
- AWS account (optional, for S3 backup storage)

### Building from Source

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/milena.git
   cd milena
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

### Running the System

#### Configuration

Both the router and cache components are configured using environment variables:

**Cache Server Configuration:**

```bash
# Required
export LISTEN_ADDR=0.0.0.0:50051
export ROUTER_ADDR=http://localhost:50050
export LRU_SIZE=10000
export TTL_SECONDS=3600
export METRICS_PORT=9091
export AWS_REGION=us-west-2
export S3_BUCKET=my-cache-bucket

# Optional
export LOG_LEVEL=info
```

**Router Configuration:**

```bash
# Required
export LISTEN_ADDR=0.0.0.0:50050
export RATE_LIMIT=100
export LOG_LEVEL=info
```

#### Starting a Router

```bash
cargo run --bin milena-router
```

#### Starting Cache Nodes

Start multiple cache nodes with different LISTEN_ADDR values:

```bash
# Terminal 1
export LISTEN_ADDR=0.0.0.0:50051
cargo run --bin milena-cache

# Terminal 2
export LISTEN_ADDR=0.0.0.0:50052
cargo run --bin milena-cache

# Terminal 3
export LISTEN_ADDR=0.0.0.0:50053
cargo run --bin milena-cache
```

### Usage Examples

Below are examples of how to interact with the Milena caching system using a gRPC client.

```rust
use milena_protos::router_server::router_client::RouterClient;
use milena_protos::router_server::{GetRequest, PutRequest, DeleteRequest};

async fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the router
    let mut client = RouterClient::connect("http://localhost:50050").await?;

    // Store a value
    let put_response = client.put(PutRequest {
        key: b"my_key".to_vec(),
        bucket: "default".to_string(),
        value: b"my_value".to_vec(),
    }).await?;

    println!("Put successful: {}", put_response.into_inner().successful);

    // Retrieve a value
    let get_response = client.get(GetRequest {
        key: b"my_key".to_vec(),
        bucket: "default".to_string(),
    }).await?;

    let response = get_response.into_inner();
    println!("Get successful: {}", response.successful);
    println!("Value: {:?}", String::from_utf8_lossy(&response.value));

    // Delete a value
    let delete_response = client.delete(DeleteRequest {
        key: b"my_key".to_vec(),
        bucket: "default".to_string(),
    }).await?;

    println!("Delete successful: {}", delete_response.into_inner().successful);

    Ok(())
}
```

## Monitoring

The cache nodes expose Prometheus metrics at `/metrics` on the configured metrics port. These metrics include:

- Cache hit/miss rates
- Request latency
- Error counts
- Memory usage

You can configure Prometheus to scrape these endpoints for monitoring and alerting.

## Project Structure

- **milena-protos**: Protocol Buffer definitions and generated gRPC code
- **milena-router**: Router implementation with consistent hashing and request routing
- **milena-cache**: Cache node implementation with multi-tiered storage

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
