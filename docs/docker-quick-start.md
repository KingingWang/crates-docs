# Docker 快速上手指南

## 一分钟开始

```bash
# 构建并运行 (生产版本)
docker build -t crates-docs:latest . && \
docker run -d -p 8080:8080 --name crates-docs crates-docs:latest

# 查看状态
docker ps
curl http://localhost:8080/health
```

---

## 文件速查

| 文件 | 用途 | 何时使用 |
|------|------|----------|
| `Dockerfile` | **主生产版本** (distroless) | 日常构建 |
| `Dockerfile.alpine` | 开发调试版本 | 需要 shell 调试 |
| `Dockerfile.scratch` | 极致最小化 | 最高安全要求 |
| `docker-compose.yml` | 服务编排 | 本地开发/测试 |
| `.dockerignore` | 构建优化 | 已配置，无需修改 |

---

## 常用命令

### 构建

```bash
# 标准构建
docker build -t crates-docs:latest .

# 使用 BuildKit (更快)
DOCKER_BUILDKIT=1 docker build -t crates-docs:latest .

# 指定 Dockerfile
docker build -f Dockerfile.alpine -t crates-docs:dev .
```

### 运行

```bash
# 基本运行
docker run -d -p 8080:8080 crates-docs:latest

# 带配置和日志卷
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  -v $(pwd)/logs:/app/logs \
  --name crates-docs \
  crates-docs:latest
```

### 管理

```bash
# 查看日志
docker logs -f crates-docs

# 停止
docker stop crates-docs

# 删除容器
docker rm crates-docs

# 删除镜像
docker rmi crates-docs:latest

# 清理所有
docker system prune -a
```

---

## Docker Compose 工作流

```bash
# 启动
docker-compose up -d

# 查看日志
docker-compose logs -f

# 重建 (源码变更后)
docker-compose up -d --build

# 停止
docker-compose down

# 完全清理
docker-compose down -v
```

---

## 故障排查

### 构建失败

```bash
# 清理缓存重建
docker build --no-cache -t crates-docs:latest .

# 查看详细输出
docker build --progress=plain -t crates-docs:latest .
```

### 容器无法启动

```bash
# 查看日志
docker logs crates-docs

# 交互式运行看错误
docker run --rm -it crates-docs:latest
```

### 无法进入容器

```bash
# distroless/scratch 没有 shell
# 方案 1: 使用 Alpine 版本调试
docker build -f Dockerfile.alpine -t crates-docs:debug .
docker run -it crates-docs:debug sh

# 方案 2: 使用 distroless debug 版本
docker run --rm -it --entrypoint=sh gcr.io/distroless/cc:debug
```

---

## 性能优化建议

### 加速构建

```bash
# 使用 BuildKit
export DOCKER_BUILDKIT=1

# 启用内联缓存
docker build \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --cache-from=crates-docs:latest \
  -t crates-docs:latest .
```

### 减小镜像

```bash
# 使用多阶段构建 (已实现)
# 使用 distroless 基础镜像 (已实现)
# 使用 scratch 终极最小化 (可选)
docker build -f Dockerfile.scratch -t crates-docs:minimal .
```

---

## 安全最佳实践

✅ **已实现**:
- 非 root 用户运行 (nobody, uid 65534)
- 最小化基础镜像 (distroless/scratch)
- 静态链接二进制
- 多阶段构建分离构建和运行时

🔒 **额外建议**:
```bash
# 使用只读文件系统
docker run --read-only -v /tmp:/tmp crates-docs:latest

# 限制资源
docker run --memory=512m --cpus=1.0 crates-docs:latest

# 禁用特权
docker run --security-opt=no-new-privileges:true crates-docs:latest
```

---

## 快速参考卡片

```bash
# 构建
DOCKER_BUILDKIT=1 docker build -t crates-docs:latest .

# 运行
docker run -d -p 8080:8080 --name crates-docs crates-docs:latest

# 查看
docker logs -f crates-docs
curl http://localhost:8080/health

# 停止
docker stop crates-docs && docker rm crates-docs

# Compose
docker-compose up -d
docker-compose logs -f
docker-compose down
```

---

## 获取帮助

- 📖 [完整优化文档](DOCKER_OPTIMIZATION.md)
- 🚀 [详细优化总结](OPTIMIZATION_SUMMARY.md)
- 🐛 [故障排查](#故障排查)

---

**完成日期**: 2026-03-29  
**优化版本**: v2.0  
**Dockerfile 数量**: 3 (distroless, scratch, alpine)
