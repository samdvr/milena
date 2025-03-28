# Milena Router

The router component of the Milena distributed caching system. The router handles client requests and directs them to the appropriate cache node using consistent hashing.

## Architecture

The router has three main responsibilities:

1. **Request Routing**: Routes client requests to the correct cache node based on key
2. **Cluster Management**: Manages the cache node membership (join/leave)
3. **Request Validation**: Validates incoming requests and enforces rate limits

## Components

### Router Service Implementation

The main service implementation in `src/service/mod.rs` provides the following API endpoints:

- **Data Operations**:

  - `get(key, bucket)`: Retrieve a value
  - `put(key, bucket, value)`: Store a value
  - `delete(key, bucket)`: Remove a value

- **Cluster Management**:
  - `join(address)`: Add a new cache node to the cluster
  - `leave(address)`: Remove a cache node from the cluster

### Consistent Hashing

The router uses consistent hashing to distribute keys across cache nodes:

- Ensures even distribution of keys
- Minimizes redistribution when nodes join or leave
- Implemented using the `conhash` crate (with the `ServerNode` wrapper)

### Connection Pool Management

The router maintains connection pools to all cache nodes:

- Implemented in `src/connection.rs` using `deadpool`
- Manages gRPC client connections to each cache node
- Handles connection pooling, recycling, and error handling

### Rate Limiting

Rate limiting is implemented in `src/rate_limit.rs`:

- Uses the `governor` crate for rate limiting
- Configurable request rate per second
- Applied to all API endpoints

### Validation

Request validation in `src/validation.rs` ensures:

- Valid bucket names
- Appropriately sized keys and values
- Valid node addresses

## Configuration

The router is configured through environment variables:

```bash
# Required
export LISTEN_ADDR=0.0.0.0:50050     # gRPC listen address
export RATE_LIMIT=100                # Requests per second
export LOG_LEVEL=info                # Logging level
```

## Node Management

### Adding a Node

When a cache node calls the `join` method:

1. The address is validated
2. The node is added to the consistent hash ring
3. A connection pool is created for the node
4. The node becomes available for routing

### Removing a Node

When a cache node calls the `leave` method or fails:

1. The node is removed from the consistent hash ring
2. The connection pool is destroyed
3. Requests for keys previously mapped to this node are redistributed

## Request Routing Process

When a client makes a data request (get/put/delete):

1. Request validation checks bucket name, key, and value (for put)
2. Rate limiting is applied
3. The key is hashed to determine the target node
4. A connection is acquired from the connection pool for that node
5. The request is forwarded to the node
6. The response is returned to the client

## Error Handling

Error handling is defined in `src/service/mod.rs` with the `RouterError` enum:

- `NodeNotFound`: No node available for a key
- `ConnectionError`: Error connecting to a cache node
- `ValidationError`: Invalid request parameters
- `RateLimitError`: Rate limit exceeded
- `InternalError`: Unexpected internal error

## Running the Router

```bash
# Set required environment variables
cargo run --bin milena-router
```

## Development

### Extending the Router

To add new functionality to the router:

1. Update the Protocol Buffers definitions in `milena-protos`
2. Implement the new methods in `RouterServiceImpl`
3. Add validation for new parameters if needed

### Testing

Run the tests with:

```bash
cargo test --bin milena-router
```
