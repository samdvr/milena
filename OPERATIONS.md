# Milena Operations Guide

This operations guide provides practical information for running, monitoring, and maintaining the Milena distributed caching system.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Deployment](#deployment)
3. [Monitoring](#monitoring)
4. [Scaling](#scaling)
5. [Troubleshooting](#troubleshooting)
6. [Backup and Recovery](#backup-and-recovery)
7. [Advanced Configuration](#advanced-configuration)

## Getting Started

### System Overview

Milena consists of a router and multiple cache nodes:

```
                     ┌─────────────┐
                     │   Client    │
                     └──────┬──────┘
                            │
                            ▼
                     ┌─────────────┐
                     │   Router    │
                     └──────┬──────┘
                            │
          ┌────────────────┼────────────────┐
          │                │                │
          ▼                ▼                ▼
┌──────────────────┐┌─────────────────┐┌─────────────────┐
│  Cache Node 1    ││   Cache Node 2  ││   Cache Node 3  │
└──────────────────┘└─────────────────┘└─────────────────┘
```

### Quick Start

1. Build the binaries:

   ```bash
   cargo build --release
   ```

2. Start the router:

   ```bash
   export LISTEN_ADDR=0.0.0.0:50050
   export RATE_LIMIT=100
   export LOG_LEVEL=info
   ./target/release/milena-router
   ```

3. Start cache node(s):
   ```bash
   export LISTEN_ADDR=0.0.0.0:50051
   export ROUTER_ADDR=http://localhost:50050
   export LRU_SIZE=10000
   export TTL_SECONDS=3600
   export METRICS_PORT=9091
   export AWS_REGION=us-west-2
   export S3_BUCKET=my-cache-bucket
   ./target/release/milena-cache
   ```

## Deployment

### System Requirements

- **Router**:
  - 2 CPU cores
  - 2GB RAM
  - 10GB disk space
- **Cache Node**:
  - 4 CPU cores
  - 8GB RAM minimum (more for larger LRU cache sizes)
  - SSD storage recommended
  - AWS credentials for S3 access

### Docker Deployment

Create a `docker-compose.yml` file:

```yaml
version: "3"

services:
  router:
    build:
      context: .
      dockerfile: Dockerfile
    command: ./milena-router
    environment:
      - LISTEN_ADDR=0.0.0.0:50050
      - RATE_LIMIT=100
      - LOG_LEVEL=info
    ports:
      - "50050:50050"

  cache1:
    build:
      context: .
      dockerfile: Dockerfile
    command: ./milena-cache
    environment:
      - LISTEN_ADDR=0.0.0.0:50051
      - ROUTER_ADDR=http://router:50050
      - LRU_SIZE=10000
      - TTL_SECONDS=3600
      - METRICS_PORT=9091
      - AWS_REGION=us-west-2
      - AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}
      - AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}
      - S3_BUCKET=my-cache-bucket
    ports:
      - "50051:50051"
      - "9091:9091"
    depends_on:
      - router

  cache2:
    build:
      context: .
      dockerfile: Dockerfile
    command: ./milena-cache
    environment:
      - LISTEN_ADDR=0.0.0.0:50052
      - ROUTER_ADDR=http://router:50050
      - LRU_SIZE=10000
      - TTL_SECONDS=3600
      - METRICS_PORT=9092
      - AWS_REGION=us-west-2
      - AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}
      - AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}
      - S3_BUCKET=my-cache-bucket
    ports:
      - "50052:50052"
      - "9092:9092"
    depends_on:
      - router
```

Start the services:

```bash
docker-compose up -d
```

### Kubernetes Deployment

Create Kubernetes deployment and service manifests for each component. Here's an example for the router:

```yaml
# router-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: milena-router
spec:
  replicas: 1
  selector:
    matchLabels:
      app: milena-router
  template:
    metadata:
      labels:
        app: milena-router
    spec:
      containers:
        - name: router
          image: milena-router:latest
          ports:
            - containerPort: 50050
          env:
            - name: LISTEN_ADDR
              value: "0.0.0.0:50050"
            - name: RATE_LIMIT
              value: "100"
            - name: LOG_LEVEL
              value: "info"
          resources:
            requests:
              memory: "1Gi"
              cpu: "500m"
            limits:
              memory: "2Gi"
              cpu: "1000m"
---
# router-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: milena-router
spec:
  selector:
    app: milena-router
  ports:
    - port: 50050
      targetPort: 50050
  type: ClusterIP
```

Apply the manifests:

```bash
kubectl apply -f router-deployment.yaml
kubectl apply -f router-service.yaml
```

## Monitoring

### Prometheus Metrics

The cache nodes expose Prometheus metrics at `/metrics` on the configured port. Configure Prometheus to scrape these endpoints:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: "milena-cache"
    scrape_interval: 15s
    static_configs:
      - targets: ["cache1:9091", "cache2:9092"]
```

### Key Metrics to Monitor

- **Cache Performance**:

  - `milena_cache_hits_total`: Total cache hits
  - `milena_cache_misses_total`: Total cache misses
  - `milena_operation_duration_seconds`: Operation latency

- **System Health**:
  - `milena_request_counter`: Total requests processed
  - `milena_error_counter`: Error count

### Grafana Dashboard

Create a Grafana dashboard to visualize the metrics. Example queries:

- Hit ratio: `sum(rate(milena_cache_hits_total[5m])) / (sum(rate(milena_cache_hits_total[5m])) + sum(rate(milena_cache_misses_total[5m])))`
- Operation latency: `histogram_quantile(0.95, sum(rate(milena_operation_duration_seconds_bucket[5m])) by (le))`
- Request rate: `sum(rate(milena_request_counter[5m]))`
- Error rate: `sum(rate(milena_error_counter[5m]))`

## Scaling

### Adding Cache Nodes

To scale out by adding more cache nodes:

1. Start a new cache node with a unique port:
   ```bash
   export LISTEN_ADDR=0.0.0.0:50053
   export ROUTER_ADDR=http://localhost:50050
   export LRU_SIZE=10000
   export TTL_SECONDS=3600
   export METRICS_PORT=9093
   export AWS_REGION=us-west-2
   export S3_BUCKET=my-cache-bucket
   ./target/release/milena-cache
   ```

The node will automatically join the cluster by calling the router's `join` method.

### Removing Cache Nodes

To gracefully remove a cache node:

1. Disconnect from the router:

   ```rust
   // Example client code
   let mut client = RouterClient::connect("http://localhost:50050").await?;
   client.leave(LeaveRequest {
       address: "http://localhost:50053".to_string(),
   }).await?;
   ```

2. Shut down the node.

### Scaling for Performance

- Increase the LRU cache size for better hit rates: `export LRU_SIZE=100000`
- Adjust TTL for cache entries based on your data update patterns: `export TTL_SECONDS=7200`

## Troubleshooting

### Common Issues

#### Cache Node Can't Connect to Router

**Symptoms**: Log shows "Failed to join router" error

**Solutions**:

- Verify router is running: `curl http://localhost:50050`
- Check network connectivity: `telnet localhost 50050`
- Ensure ROUTER_ADDR is correct

#### High Latency

**Symptoms**: Slow response times in metrics

**Solutions**:

- Check S3 connectivity
- Increase LRU cache size
- Add more cache nodes
- Monitor system resources (CPU, memory, disk I/O)

#### Low Cache Hit Rate

**Symptoms**: High ratio of cache misses to hits

**Solutions**:

- Increase LRU cache size
- Adjust TTL settings
- Review access patterns for hot keys

### Log Analysis

Analyze logs to identify issues:

```bash
# Filter router logs for errors
grep "error" router.log

# View cache join/leave events
grep "Join" router.log
```

## Backup and Recovery

### S3 Data Management

S3 serves as the backup tier for cached data. To manage S3 data:

1. List bucket contents:

   ```bash
   aws s3 ls s3://my-cache-bucket/ --recursive
   ```

2. Backup S3 data to another bucket:
   ```bash
   aws s3 sync s3://my-cache-bucket/ s3://my-backup-bucket/
   ```

### Cache Node Recovery

If a cache node fails:

1. Start a new cache node with the same configuration
2. It will automatically join the cluster and begin serving requests
3. Data will be populated from S3 as needed

## Advanced Configuration

### Customizing Rate Limits

Adjust the router's rate limit based on your workload:

```bash
export RATE_LIMIT=500  # Allow 500 requests per second
```

### Multi-region Deployment

For multi-region deployments:

1. Deploy a router and multiple cache nodes in each region
2. Configure clients to connect to their nearest router
3. Use a shared S3 bucket or region-specific buckets

### Secure Communication

To enable TLS:

1. Generate TLS certificates
2. Configure the router and cache nodes to use TLS
3. Update client connections to use HTTPS URLs

Example configuration:

```rust
// Server-side TLS configuration
Server::builder()
    .tls_config(tls_config)?
    .add_service(RouterServer::new(router_service))
    .serve(addr)
    .await?;

// Client-side TLS configuration
let channel = Channel::from_static("https://example.com:50050")
    .tls_config(tls_config)?
    .connect()
    .await?;
let client = RouterClient::new(channel);
```
