# Crates Docs MCP 服务器

一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议和 OAuth 认证。

## 特性

- 🚀 **高性能**: 使用异步 Rust 和智能缓存
- 🔧 **多种传输协议**: 支持 Stdio、HTTP 和 SSE
- 🔐 **OAuth 认证**: 支持 GitHub、Google、Keycloak 等
- 📚 **完整的文档查询**: 支持查找 crate、搜索 crate、查找特定项目
- 🛡️ **安全**: 支持速率限制、连接池和请求验证
- 📊 **监控**: 内置健康检查和性能监控
- ⚙️ **可配置**: 灵活的配置文件和环境变量支持

## 快速开始

### 安装

```bash
# 克隆仓库
git clone <repository-url>
cd crates-docs

# 构建项目
cargo build --release

# 运行服务器
cargo run -- serve
```

### 使用 Docker

```bash
# 构建 Docker 镜像
docker build -t crates-docs .

# 运行容器
docker run -p 8080:8080 crates-docs
```

## 使用方法

### 启动服务器

```bash
# 使用默认配置启动服务器（混合模式：HTTP + SSE）
cargo run -- serve

# 使用 Stdio 模式（用于 CLI 集成）
cargo run -- serve --mode stdio

# 使用 HTTP 模式
cargo run -- serve --mode http --host 0.0.0.0 --port 8080

# 启用调试日志
cargo run -- serve --debug

# 启用详细输出
cargo run -- serve --verbose

# 使用自定义配置文件
cargo run -- serve --config /path/to/config.toml
```

### 生成配置文件

```bash
# 生成默认配置文件
cargo run -- config --output config.toml

# 覆盖已存在的配置文件
cargo run -- config --output config.toml --force
```

### 测试工具

```bash
# 测试查找 crate
cargo run -- test --tool lookup_crate --crate-name serde

# 测试搜索 crate
cargo run -- test --tool search_crates --query "web framework" --limit 5

# 测试查找项目
cargo run -- test --tool lookup_item --crate-name serde --item-path "serde::Serialize"

# 测试健康检查
cargo run -- test --tool health_check
```

### 健康检查

```bash
# 执行健康检查
cargo run -- health

# 详细输出
cargo run -- health --verbose

# 检查特定服务
cargo run -- health --check-type external
```

## 配置

### 配置文件示例

创建 `config.toml`：

```toml
[server]
name = "crates-docs"
version = "0.1.0"
description = "高性能 Rust crate 文档查询 MCP 服务器"
host = "127.0.0.1"
port = 8080
transport_mode = "hybrid"
enable_sse = true
enable_oauth = false
max_connections = 100
request_timeout_secs = 30
response_timeout_secs = 60

[cache]
cache_type = "memory"
memory_size = 1000
redis_url = null
default_ttl = 3600

[oauth]
enabled = false
client_id = ""
client_secret = ""
redirect_uri = ""
authorization_endpoint = ""
token_endpoint = ""
scopes = ["openid", "profile", "email"]
provider = "Custom"

[logging]
level = "info"
file_path = "./logs/crates-docs.log"
enable_console = true
enable_file = true
max_file_size_mb = 100
max_files = 10

[performance]
http_client_pool_size = 10
cache_max_size = 1000
cache_default_ttl_secs = 3600
rate_limit_per_second = 100
concurrent_request_limit = 50
enable_response_compression = true
```

### 环境变量

所有配置都可以通过环境变量覆盖：

```bash
export CRATES_DOCS_HOST="0.0.0.0"
export CRATES_DOCS_PORT="8080"
export CRATES_DOCS_TRANSPORT_MODE="http"
export CRATES_DOCS_LOG_LEVEL="debug"
```

## MCP 工具

### 可用工具

1. **lookup_crate** - 查找 crate 文档
   - `crate_name`: crate 名称（必需）
   - `version`: 版本号（可选）
   - `format`: 输出格式（markdown/text/html，默认：markdown）

2. **search_crates** - 搜索 crate
   - `query`: 搜索关键词（必需）
   - `limit`: 结果数量限制（1-100，默认：10）
   - `format`: 输出格式（markdown/text/json，默认：markdown）

3. **lookup_item** - 查找 crate 中的特定项目
   - `crate_name`: crate 名称（必需）
   - `itemPath`: 项目路径（如 std::vec::Vec）（必需）
   - `version`: 版本号（可选）

4. **health_check** - 健康检查
   - `checkType`: 检查类型（all/external/internal/docs_rs/crates_io，默认：all）
   - `verbose`: 详细输出（true/false，默认：false）

## 传输协议

### Stdio 模式

用于 CLI 工具集成：

```bash
# 通过 Stdio 运行
cargo run -- serve --mode stdio

# 使用 MCP Inspector 测试
npx @modelcontextprotocol/inspector cargo run -- serve --mode stdio
```

### HTTP 模式（Streamable HTTP）

用于网络服务：

```bash
# 启动 HTTP 服务器
cargo run -- serve --mode http --host 0.0.0.0 --port 8080

# 使用 curl 测试
curl http://localhost:8080/health
```

### SSE 模式（Server-Sent Events）

用于向后兼容（已弃用，推荐使用 Hybrid 模式）：

```bash
# 启动 SSE 服务器
cargo run -- serve --mode sse --host 0.0.0.0 --port 8080
```

### 混合模式（HTTP + SSE）

推荐模式，同时支持 Streamable HTTP 和 Server-Sent Events 通信：

```bash
# 启动混合服务器
cargo run -- serve --mode hybrid --host 0.0.0.0 --port 8080
```

## OAuth 认证

### 启用 OAuth

1. 在配置文件中启用 OAuth：

```toml
[oauth]
enabled = true
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "http://localhost:8080/oauth/callback"
authorization_endpoint = "https://provider.com/oauth/authorize"
token_endpoint = "https://provider.com/oauth/token"
scopes = ["openid", "profile", "email"]
provider = "Custom"
```

2. 或使用预配置的提供者：

```bash
# GitHub OAuth
cargo run -- serve --enable-oauth \
  --oauth-client-id "github-client-id" \
  --oauth-client-secret "github-client-secret" \
  --oauth-redirect-uri "http://localhost:8080/oauth/callback"

# Google OAuth
cargo run -- serve --enable-oauth \
  --oauth-client-id "google-client-id" \
  --oauth-client-secret "google-client-secret" \
  --oauth-redirect-uri "http://localhost:8080/oauth/callback"
```

### 支持的 OAuth 提供者

- **GitHub**: `provider = "GitHub"`
- **Google**: `provider = "Google"`
- **Keycloak**: `provider = "Keycloak"`
- **自定义**: `provider = "Custom"`

## 性能优化

### 缓存

支持内存缓存和 Redis 缓存：

```toml
[cache]
cache_type = "memory"  # 或 "redis"
memory_size = 1000     # 内存缓存条目数
redis_url = "redis://localhost:6379"  # Redis 连接 URL
default_ttl = 3600     # 默认缓存时间（秒）
```

### 连接池

```toml
[performance]
http_client_pool_size = 10           # HTTP 客户端连接池大小
concurrent_request_limit = 50        # 并发请求限制
rate_limit_per_second = 100          # 每秒请求限制
enable_response_compression = true   # 启用响应压缩
```

## 开发

### 构建

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 检查代码
cargo check
cargo clippy
cargo fmt
```

### 测试

```bash
# 运行单元测试
cargo test

# 运行集成测试
cargo test --test integration_tests

# 运行特定测试
cargo test test_lookup_crate
```

### 代码质量

```bash
# 代码格式化
cargo fmt

# 代码检查
cargo clippy -- -D warnings

# 安全检查
cargo audit
```

## 部署

### 系统服务（Systemd）

创建 `/etc/systemd/system/crates-docs.service`：

```ini
[Unit]
Description=Crates Docs MCP Server
After=network.target

[Service]
Type=simple
User=crates-docs
WorkingDirectory=/opt/crates-docs
ExecStart=/opt/crates-docs/target/release/crates-docs serve --config /etc/crates-docs/config.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Docker Compose

项目包含完整的 `docker-compose.yml`，支持以下服务：

```yaml
version: '3.8'

services:
  crates-docs:    # 主服务
    build: .
    ports:
      - "8080:8080"
  
  redis:          # Redis 缓存服务
    image: redis:7-alpine
    ports:
      - "6379:6379"
  
  prometheus:     # Prometheus 监控（可选）
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
  
  grafana:        # Grafana 仪表板（可选）
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
```

启动所有服务：
```bash
docker-compose up -d
```

仅启动核心服务（不包含监控）：
```bash
docker-compose up -d crates-docs redis
```

## API 文档

### 健康检查端点

```
GET /health
```

响应：
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z",
  "checks": [
    {
      "name": "docs.rs",
      "status": "healthy",
      "duration_ms": 123,
      "message": "服务正常"
    }
  ],
  "uptime": "1h 30m 15s"
}
```

### MCP 端点

- `POST /mcp` - MCP Streamable HTTP 端点
- `GET /mcp/sse` - MCP SSE 端点

## 故障排除

### 常见问题

1. **端口被占用**
   ```bash
   # 检查端口占用
   sudo lsof -i :8080
   
   # 杀死占用进程
   sudo kill -9 <PID>
   ```

2. **内存不足**
   ```bash
   # 调整缓存大小
   [cache]
   memory_size = 500  # 减少缓存大小
   ```

3. **网络问题**
   ```bash
   # 检查网络连接
   curl -I https://docs.rs/
   curl -I https://crates.io/api/v1/crates?q=test&per_page=1
   ```

### 日志

日志文件位于 `./logs/crates-docs.log`，可以通过配置调整日志级别：

```toml
[logging]
level = "debug"  # trace, debug, info, warn, error
```

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开 Pull Request

## 致谢

- [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) - MCP SDK
- [docs.rs](https://docs.rs) - Rust 文档服务
- [crates.io](https://crates.io) - Rust 包注册表

## 支持

如有问题，请：
1. 查看 [Issues](https://github.com/KingingWang/crates-docs/issues)
2. 查看 [文档](https://github.com/KingingWang/crates-docs/wiki)
3. 发送邮件到 kingingwang@foxmail.com