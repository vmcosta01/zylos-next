# Deployment Guide

## Prerequisites

- Rust 1.80+ (stable toolchain)
- Docker 24+ (for sandbox isolation and containerized deployment)
- SQLite 3.40+ (bundled via rusqlite, no external install needed)
- 4GB RAM minimum (~30MB per agent idle, ~200MB base overhead)
- Kubernetes 1.28+ (optional, for orchestrated deployments)
- Helm 3.12+ (optional, for chart-based K8s deployment)

## Quick Deploy (Bare Metal)

### Build from Source

```bash
# Clone repository
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next

# Build release binary with optimizations
cargo build --release -p zylos-gateway

# Verify binary
./target/release/zylos-gateway --version
# zylos-gateway 0.1.0
```

### Create Configuration

```bash
cat > config.toml << 'EOF'
[server]
listen_addr = "0.0.0.0:3000"
shutdown_timeout_secs = 5
max_connections = 1000

[health]
ready_enabled = true
live_enabled = true

[soar]
audit_key = "change-me-in-production"
event_bus_capacity = 256
incident_rate_limit = 5

[skills]
index_path = "./skills"
auto_reload = true

[memory]
db_path = "data/memory.db"
consolidation_interval_secs = 3600
decay_half_life_days = 30

[sandbox]
wasm_enabled = true
docker_enabled = true
browser_enabled = false
EOF
```

### Run as Systemd Service

Create `/etc/systemd/system/zylos-gateway.service`:

```ini
[Unit]
Description=Zylos Gateway
After=network.target docker.service
Wants=docker.service

[Service]
Type=simple
User=zylos
Group=zylos
WorkingDirectory=/opt/zylos
ExecStart=/opt/zylos/bin/zylos-gateway --config /etc/zylos/config.toml
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable zylos-gateway
sudo systemctl start zylos-gateway
sudo systemctl status zylos-gateway
```

### Verify Deployment

```bash
# Health check
curl http://localhost:3000/gateway/health
# {"service":"zylos-gateway","status":"healthy","version":"0.1.0"}

# Readiness probe
curl http://localhost:3000/gateway/ready
# {"status":"ready","subsystems":{"memory":"ok","orchestrator":"ok","skills":"ok"}}

# Liveness probe
curl http://localhost:3000/gateway/live
# {"status":"alive","uptime_secs":42}
```

## Docker Deploy

### Single Container

Build the image:

```dockerfile
# Dockerfile
FROM rust:1.80-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p zylos-gateway

FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y ca-certificates sqlite3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/zylos-gateway /usr/local/bin/
COPY --from=builder /app/skills /opt/zylos/skills

RUN mkdir -p /data /etc/zylos
EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
  CMD curl -f http://localhost:3000/gateway/health || exit 1

CMD ["zylos-gateway", "--config", "/etc/zylos/config.toml"]
```

Build and run:

```bash
docker build -t zylos-gateway:latest .

docker run -d \
  --name zylos-gateway \
  -p 3000:3000 \
  -v $(pwd)/config.toml:/etc/zylos/config.toml:ro \
  -v zylos-data:/data \
  -e RUST_LOG=info \
  --restart unless-stopped \
  zylos-gateway:latest
```

### Docker Compose

```yaml
# docker-compose.yml
version: "3.9"

services:
  gateway:
    image: zylos-gateway:latest
    ports:
      - "3000:3000"
    volumes:
      - ./config.toml:/etc/zylos/config.toml:ro
      - zylos-data:/data
      - ./skills:/opt/zylos/skills:ro
    environment:
      - RUST_LOG=info
      - ZYLOS_SOAR_AUDIT_KEY=${SOAR_AUDIT_KEY}
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/gateway/health"]
      interval: 10s
      timeout: 3s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: "2.0"
        reservations:
          memory: 256M

volumes:
  zylos-data:
```

Run with compose:

```bash
export SOAR_AUDIT_KEY="your-production-key"
docker compose up -d
docker compose logs -f gateway
```

### Multi-Stage Build Optimization

For smaller images, use musl-based static builds:

```dockerfile
FROM rust:1.80-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release -p zylos-gateway --target x86_64-unknown-linux-musl

FROM alpine:3.19
RUN apk add --no-cache ca-certificates curl
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/zylos-gateway /usr/local/bin/
EXPOSE 3000
CMD ["zylos-gateway"]
```

This produces an image under 20MB.

## Kubernetes Deployment

### Helm Chart

```bash
helm repo add zylos https://charts.nousresearch.com
helm repo update

helm install zylos zylos/gateway \
  --namespace zylos --create-namespace \
  --set server.listenAddr="0.0.0.0:3000" \
  --set resources.memory="512Mi" \
  --set replicas=3 \
  --set soar.auditKeySecret="zylos-soar-key" \
  --set persistence.enabled=true \
  --set persistence.size=10Gi
```

### Manual Manifests

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zylos-gateway
  labels:
    app: zylos-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: zylos-gateway
  template:
    metadata:
      labels:
        app: zylos-gateway
    spec:
      containers:
        - name: gateway
          image: ghcr.io/nousresearch/zylos-gateway:latest
          ports:
            - containerPort: 3000
          env:
            - name: RUST_LOG
              value: "info"
            - name: ZYLOS_SOAR_AUDIT_KEY
              valueFrom:
                secretKeyRef:
                  name: zylos-secrets
                  key: soar-audit-key
          resources:
            requests:
              memory: "256Mi"
              cpu: "250m"
            limits:
              memory: "512Mi"
              cpu: "1000m"
          livenessProbe:
            httpGet:
              path: /gateway/live
              port: 3000
            initialDelaySeconds: 5
            periodSeconds: 10
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /gateway/ready
              port: 3000
            initialDelaySeconds: 10
            periodSeconds: 5
            failureThreshold: 3
          volumeMounts:
            - name: data
              mountPath: /data
            - name: config
              mountPath: /etc/zylos
              readOnly: true
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: zylos-data
        - name: config
          configMap:
            name: zylos-config
---
apiVersion: v1
kind: Service
metadata:
  name: zylos-gateway
spec:
  selector:
    app: zylos-gateway
  ports:
    - port: 80
      targetPort: 3000
  type: ClusterIP
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: zylos-data
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
```

Apply manifests:

```bash
kubectl apply -f k8s/deployment.yaml
kubectl rollout status deployment/zylos-gateway -n zylos
kubectl get pods -n zylos -w
```

### Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: zylos-gateway-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: zylos-gateway
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 80
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZYLOS_LISTEN` | `0.0.0.0:3000` | Listen address and port |
| `ZYLOS_SHUTDOWN_TIMEOUT_SECS` | `5` | Graceful shutdown timeout |
| `ZYLOS_EVENT_BUS_CAPACITY` | `256` | EventBus channel buffer size |
| `ZYLOS_SKILLS_PATH` | `./skills` | Skills registry directory |
| `ZYLOS_SOAR_AUDIT_KEY` | `zylos-gateway-soar-key` | SOAR audit trail signing key |
| `ZYLOS_MEMORY_DB` | `data/memory.db` | SQLite database path |
| `ZYLOS_MAX_CONNECTIONS` | `1000` | Maximum concurrent connections |
| `ZYLOS_CONSOLIDATION_INTERVAL` | `3600` | Auto-consolidation period in seconds |
| `ZYLOS_DECAY_HALF_LIFE_DAYS` | `30` | Memory temporal decay half-life |
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |

## Production Checklist

### Security

- [ ] Change `ZYLOS_SOAR_AUDIT_KEY` from default value
- [ ] Enable TLS termination (reverse proxy or native TLS)
- [ ] Restrict `/gateway/health` endpoint to internal networks
- [ ] Set file permissions on config.toml (600) and data directory (700)
- [ ] Use secrets manager for audit keys in K8s (not plaintext env vars)
- [ ] Enable sandbox isolation (WASM + Docker tiers)

### Reliability

- [ ] Configure log rotation (`RUST_LOG=info,zylos=debug`)
- [ ] Set resource limits (memory: 30MB x expected agents + 200MB base)
- [ ] Enable readiness and liveness probes
- [ ] Configure graceful shutdown timeout >= 5 seconds
- [ ] Set up automated SQLite backups (daily recommended)
- [ ] Monitor `/gateway/health` endpoint with external uptime checker

### Performance

- [ ] Review rate limits for incident reporting (default: 5/hr per signature)
- [ ] Tune EventBus capacity for expected event throughput
- [ ] Enable memory consolidation and decay for long-running instances
- [ ] Set `max_connections` based on expected concurrent users
- [ ] Profile startup time (target: <3s cold start)

### Monitoring

- [ ] Set up log aggregation (structured JSON logging)
- [ ] Configure alerting on health endpoint failures
- [ ] Monitor memory usage trends (detect leaks early)
- [ ] Track approval card latency (p99 target: <100ms)
- [ ] Monitor triage fallback rate (high rate indicates rule gaps)

## Performance Targets

| Metric | Target | Verified |
|--------|--------|----------|
| Startup time | <3s | Yes |
| Health check latency | <5ms | Yes |
| Memory per agent (idle) | ~30MB | Yes |
| Base memory overhead | ~200MB | Yes |
| Approval card p99 | <100ms | Yes |
| Triage rules p99 | <10ms | Yes |
| Concurrent connections | 1000+ | Yes |
| Events/sec throughput | 5000+ | Yes |

## Backup and Recovery

### SQLite Backup

```bash
# Online backup (non-blocking)
sqlite3 data/memory.db ".backup '/backups/memory-$(date +%Y%m%d).db'"

# Automated daily backup via cron
echo "0 2 * * * sqlite3 /opt/zylos/data/memory.db '.backup /backups/memory-\$(date +\%Y\%m\%d).db'" | crontab -
```

### Disaster Recovery

1. Stop the gateway: `systemctl stop zylos-gateway`
2. Restore database: `cp /backups/memory-YYYYMMDD.db data/memory.db`
3. Verify integrity: `sqlite3 data/memory.db "PRAGMA integrity_check;"`
4. Restart: `systemctl start zylos-gateway`
5. Validate: `curl http://localhost:3000/gateway/health`

## Upgrading

### Rolling Upgrade (K8s)

```bash
helm upgrade zylos zylos/gateway \
  --set image.tag=v0.2.0 \
  --wait --timeout=300s
```

### Bare Metal Upgrade

```bash
# Build new version
cargo build --release -p zylos-gateway

# Stop service
sudo systemctl stop zylos-gateway

# Backup database
sqlite3 data/memory.db ".backup '/backups/pre-upgrade.db'"

# Replace binary
cp target/release/zylos-gateway /opt/zylos/bin/zylos-gateway

# Start service
sudo systemctl start zylos-gateway

# Verify
curl http://localhost:3000/gateway/health
```

Migration runs automatically on first startup if schema changes are detected.

## Troubleshooting

### Port Already in Use

```bash
ss -tlnp | grep 3000
# Kill existing process or change listen_addr in config
```

### Database Locked

```bash
# Check for zombie processes
ps aux | grep zylos

# Force WAL checkpoint
sqlite3 data/memory.db "PRAGMA wal_checkpoint(TRUNCATE);"
```

### High Memory Usage

```bash
# Check active agents
curl http://localhost:3000/api/v1/agents | jq 'length'

# Force consolidation
./target/release/zylos-gateway --force-consolidation

# Enable debug logging for memory subsystem
RUST_LOG=zylos_memory=debug ./target/release/zylos-gateway
```

### Sandbox Failures

```bash
# Check Docker availability
docker info

# Verify WASM runtime
./target/release/zylos-gateway --check-sandbox

# Disable sandbox tiers if needed (not recommended for production)
# Set wasm_enabled = false, docker_enabled = false in config.toml
```
