# Docker Quick Start Guide

## One-Minute Start

```bash
# Build and run (production version)
docker build -t crates-docs:latest . && \
docker run -d -p 8080:8080 --name crates-docs crates-docs:latest

# Check status
docker ps
curl http://localhost:8080/health
```

---

## File Reference

| File | Purpose | When to Use |
|------|---------|-------------|
| `Dockerfile` | **Main production version** (distroless) | Daily builds |
| `Dockerfile.alpine` | Development/debug version | When shell debugging is needed |
| `Dockerfile.scratch` | Ultra-minimal version | Highest security requirements |
| `docker-compose.yml` | Service orchestration | Local development/testing |
| `.dockerignore` | Build optimization | Already configured, no modification needed |

---

## Common Commands

### Build

```bash
# Standard build
docker build -t crates-docs:latest .

# Using BuildKit (faster)
DOCKER_BUILDKIT=1 docker build -t crates-docs:latest .

# Specify Dockerfile
docker build -f Dockerfile.alpine -t crates-docs:dev .
```

### Run

```bash
# Basic run
docker run -d -p 8080:8080 crates-docs:latest

# With config and log volumes
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  -v $(pwd)/logs:/app/logs \
  --name crates-docs \
  crates-docs:latest
```

### Manage

```bash
# View logs
docker logs -f crates-docs

# Stop
docker stop crates-docs

# Remove container
docker rm crates-docs

# Remove image
docker rmi crates-docs:latest

# Clean all
docker system prune -a
```

---

## Docker Compose Workflow

```bash
# Start
docker-compose up -d

# View logs
docker-compose logs -f

# Rebuild (after source changes)
docker-compose up -d --build

# Stop
docker-compose down

# Full cleanup
docker-compose down -v
```

---

## Troubleshooting

### Build Failed

```bash
# Rebuild without cache
docker build --no-cache -t crates-docs:latest .

# View detailed output
docker build --progress=plain -t crates-docs:latest .
```

### Container Won't Start

```bash
# View logs
docker logs crates-docs

# Run interactively to see errors
docker run --rm -it crates-docs:latest
```

### Cannot Enter Container

```bash
# distroless/scratch has no shell
# Option 1: Use Alpine version for debugging
docker build -f Dockerfile.alpine -t crates-docs:debug .
docker run -it crates-docs:debug sh

# Option 2: Use distroless debug image
docker run --rm -it --entrypoint=sh gcr.io/distroless/cc:debug
```

---

## Performance Optimization Tips

### Speed Up Builds

```bash
# Use BuildKit
export DOCKER_BUILDKIT=1

# Enable inline cache
docker build \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --cache-from=crates-docs:latest \
  -t crates-docs:latest .
```

### Reduce Image Size

```bash
# Use multi-stage builds (already implemented)
# Use distroless base image (already implemented)
# Use scratch for ultimate minimal size (optional)
docker build -f Dockerfile.scratch -t crates-docs:minimal .
```

---

## Security Best Practices

✅ **Implemented**:
- Non-root user execution (nobody, uid 65534)
- Minimal base image (distroless/scratch)
- Statically linked binary
- Multi-stage build separates build and runtime

🔒 **Additional Recommendations**:
```bash
# Use read-only filesystem
docker run --read-only -v /tmp:/tmp crates-docs:latest

# Limit resources
docker run --memory=512m --cpus=1.0 crates-docs:latest

# Disable privileges
docker run --security-opt=no-new-privileges:true crates-docs:latest
```

---

## Quick Reference Card

```bash
# Build
DOCKER_BUILDKIT=1 docker build -t crates-docs:latest .

# Run
docker run -d -p 8080:8080 --name crates-docs crates-docs:latest

# Check
docker logs -f crates-docs
curl http://localhost:8080/health

# Stop
docker stop crates-docs && docker rm crates-docs

# Compose
docker-compose up -d
docker-compose logs -f
docker-compose down
```

---

## Get Help

- 📖 [Full Optimization Documentation](DOCKER_OPTIMIZATION.md)
- 🚀 [Detailed Optimization Summary](OPTIMIZATION_SUMMARY.md)
- 🐛 [Troubleshooting](#troubleshooting)

---

**Completion Date**: 2026-03-29  
**Optimized Version**: v2.0  
**Dockerfile Count**: 3 (distroless, scratch, alpine)