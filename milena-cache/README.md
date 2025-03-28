# Milena Cache Node

The cache node component of the Milena distributed caching system. Each cache node provides a multi-tiered storage system with an in-memory LRU cache, disk-based persistence, and S3 backup.

## Architecture

The cache node uses a tiered architecture for data storage:

1. **LRU Memory Cache**: Fast, in-memory storage with LRU eviction policy
2. **Disk Store**: Persistent storage on local disk
3. **S3 Store**: Cloud storage for durability and scalability

## Components

### Service Layer

The gRPC service implementation in `src/service/mod.rs` provides the external API:

- `get(key, bucket)`: Retrieve a value
- `put(key, bucket, value)`: Store a value
- `delete(key, bucket)`: Remove a value

### Operation Layer

The operation layer in `src/operation/mod.rs` orchestrates the three storage tiers:

1. First checks the in-memory LRU cache
2. If not found, checks the disk store
3. If still not found, checks the S3 store
4. On writes, updates all three tiers

### Storage Implementations

- **LRU Store**: An in-memory cache with a configurable capacity and LRU eviction policy
- **Disk Store**: Persistent storage using the local filesystem, with TTL support
- **S3 Store**: AWS S3-backed storage for durability and backup

### Metrics

The metrics system in `src/metrics.rs` tracks:

- Cache hits and misses
- Operation durations
- Request counts
- Error counts

Metrics are exposed through a Prometheus endpoint at `/metrics`.

## Configuration

The cache node is configured through environment variables defined in `src/config.rs`:

```bash
# Required
export LISTEN_ADDR=0.0.0.0:50051     # gRPC listen address
export ROUTER_ADDR=http://localhost:50050  # Router address to join
export LRU_SIZE=10000                # Memory cache capacity
export TTL_SECONDS=3600              # Time-to-live for cached items
export METRICS_PORT=9091             # Prometheus metrics port
export AWS_REGION=us-west-2          # AWS region for S3 storage
export S3_BUCKET=my-cache-bucket     # S3 bucket name

# Optional
export LOG_LEVEL=info                # Logging level
```

## Startup Process

1. Reads configuration from environment variables
2. Initializes logging and metrics
3. Sets up AWS S3 client
4. Creates the cache service with the three-tiered storage
5. Starts the metrics server on a separate port
6. Starts the gRPC server for handling cache operations
7. Registers with the router to join the cache cluster
8. Waits for shutdown signal (Ctrl+C) or errors

## Error Handling

The error handling is defined in `src/error.rs` with specific error types for different scenarios:

- `StoreError`: Errors from the storage layer
- `KeyNotFound`: Key doesn't exist
- `InvalidInput`: Invalid parameters
- `RateLimitExceeded`: Too many requests
- `S3Error`: AWS S3-related errors
- `RouterError`: Communication errors with the router
- `InternalError`: Unexpected internal errors

## Running the Cache Node

```bash
# Set required environment variables
cargo run --bin milena-cache
```

## Development

### Adding a New Storage Backend

To add a new storage backend:

1. Implement the `Store` trait in `src/store/mod.rs`
2. Update the `Operation` struct to use the new store
3. Update the configuration if necessary

### Testing

Run the tests with:

```bash
cargo test --bin milena-cache
```
