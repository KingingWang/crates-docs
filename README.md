# Crates Docs MCP 服务器

[![Crates.io](https://img.shields.io/crates/v/crates-docs.svg)](https://crates.io/crates/crates-docs)
[![Documentation](https://docs.rs/crates-docs/badge.svg)](https://docs.rs/crates-docs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/KingingWang/crates-docs/workflows/CI/badge.svg)](https://github.com/KingingWang/crates-docs/actions)

一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议和 OAuth 认证。

## 目录

- [特性](#特性)
- [快速开始](#快速开始)
  - [安装](#安装)
- [MCP 客户端集成指南](#mcp-客户端集成指南)
- [MCP 工具详解](#mcp-工具详解)
- [使用示例](#使用示例)
- [命令行使用](#命令行使用)
- [配置](#配置)
- [传输协议](#传输协议)
- [开发](#开发)
- [部署](#部署)
- [API 端点](#api-端点)
- [故障排除](#故障排除)
- [许可证](#许可证)
- [贡献](#贡献)
- [致谢](#致谢)
- [支持](#支持)

## 特性

- 🚀 **高性能**: 使用异步 Rust 和 LRU 智能缓存
- 🔧 **多种传输协议**: 支持 Stdio、HTTP 和 SSE
- 🔐 **OAuth 认证**: 支持 GitHub、Google、Keycloak 等
- 📚 **完整的文档查询**: 支持查找 crate、搜索 crate、查找特定项目
- 🛡️ **安全**: 支持速率限制、连接池和请求验证
- 📊 **监控**: 内置健康检查和性能监控
- ⚙️ **可配置**: 灵活的配置文件和环境变量支持

## 快速开始

### 安装
#### 下载二进制文件
从github release中获取二进制文件

#### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/KingingWang/crates-docs.git
cd crates-docs

# 构建项目
cargo build --release

# 二进制文件位于 target/release/crates-docs
```

#### 使用 Docker

```bash
# 构建 Docker 镜像
docker build -t crates-docs .

# 运行容器（默认读取容器内 /app/config.toml，并监听 0.0.0.0:8080）
docker run -p 8080:8080 crates-docs

# 使用宿主机配置文件覆盖默认配置
# 注意：容器内固定使用 /app/config.toml
# 如需自定义，请挂载到该路径
docker run -p 8080:8080 \
  -v $(pwd)/examples/config.example.toml:/app/config.toml:ro \
  crates-docs
```

#### 从 crates.io 安装

```bash
cargo install crates-docs
```

## MCP 客户端集成指南

### 在 Claude Desktop 中使用

编辑 Claude Desktop 配置文件：

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

添加以下配置：

```json
{
  "mcpServers": {
    "crates-docs": {
      "command": "/path/to/crates-docs",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

### 在 Cursor 中使用

编辑 Cursor 配置文件 `~/.cursor/mcp.json`：

```json
{
  "mcpServers": {
    "crates-docs": {
      "command": "/path/to/crates-docs",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

### 在 Windsurf 中使用

编辑 Windsurf 配置文件 `~/.codeium/windsurf/mcp_config.json`：

```json
{
  "mcpServers": {
    "crates-docs": {
      "command": "/path/to/crates-docs",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

### 在 Zed 中使用

编辑 Zed 配置文件 `~/.config/zed/settings.json`：

```json
{
  "mcp_servers": {
    "crates-docs": {
      "command": "/path/to/crates-docs",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

### 使用 HTTP 模式

如果需要通过网络访问，可以使用 HTTP 模式：

```bash
# 启动 HTTP 服务器
cargo run -- serve --mode http --host 0.0.0.0 --port 8080
```

然后在 MCP 客户端中配置：

```json
{
  "mcpServers": {
    "crates-docs": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

## MCP 工具详解

### 1. lookup_crate - 查找 Crate 文档

从 docs.rs 获取 Rust crate 的完整文档。

**参数**：

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `crate_name` | string | ✅ | Crate 名称，如 `serde`、`tokio` |
| `version` | string | ❌ | 版本号，默认最新版本，如 `1.0.0` |
| `format` | string | ❌ | 输出格式：`markdown`（默认）、`text`、`html` |

**示例**：

```json
// 查找最新版本
{ "crate_name": "serde" }

// 查找特定版本
{ "crate_name": "tokio", "version": "1.35.0" }

// 获取纯文本格式
{ "crate_name": "reqwest", "format": "text" }
```

### 2. search_crates - 搜索 Crate

从 crates.io 搜索 Rust crate。

**参数**：

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `query` | string | ✅ | 搜索关键词 |
| `limit` | number | ❌ | 结果数量（1-100），默认 10 |
| `format` | string | ❌ | 输出格式：`markdown`（默认）、`text`、`json` |

**示例**：

```json
// 基本搜索
{ "query": "web framework" }

// 限制结果数量
{ "query": "async runtime", "limit": 5 }

// 获取 JSON 格式
{ "query": "serialization", "format": "json" }
```

### 3. lookup_item - 查找特定项目文档

在指定 crate 中查找特定类型、函数或模块的文档。

**参数**：

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `crate_name` | string | ✅ | Crate 名称 |
| `item_path` | string | ✅ | 项目路径，如 `serde::Serialize`、`std::collections::HashMap` |
| `version` | string | ❌ | 版本号 |
| `format` | string | ❌ | 输出格式：`markdown`（默认）、`text` |

**示例**：

```json
// 查找 serde 的 Serialize trait
{ "crate_name": "serde", "item_path": "serde::Serialize" }

// 查找 tokio 的 run 函数
{ "crate_name": "tokio", "item_path": "tokio::runtime::Runtime::run" }

// 查找特定版本的 HashMap
{ "crate_name": "std", "item_path": "std::collections::HashMap", "version": "1.75.0" }
```

### 4. health_check - 健康检查

检查服务器和外部服务的健康状态。

**参数**：

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `check_type` | string | ❌ | 检查类型：`all`（默认）、`external`、`internal`、`docs_rs`、`crates_io` |
| `verbose` | boolean | ❌ | 详细输出，默认 false |

**示例**：

```json
// 完整检查
{ "check_type": "all", "verbose": true }

// 只检查外部服务
{ "check_type": "external" }

// 只检查 docs.rs
{ "check_type": "docs_rs" }
```

## 使用示例

### 示例 1: 了解一个新 crate

**用户**: "帮我了解一下 serde 这个 crate"

**AI 会使用**:
```json
{ "crate_name": "serde" }
```

### 示例 2: 查找特定功能

**用户**: "tokio 怎么创建一个异步任务？"

**AI 会使用**:
```json
{ "crate_name": "tokio", "item_path": "tokio::spawn" }
```

### 示例 3: 搜索相关 crate

**用户**: "有什么好用的 HTTP 客户端库？"

**AI 会使用**:
```json
{ "query": "http client", "limit": 10 }
```

### 示例 4: 比较版本差异

**用户**: "reqwest 0.11 和 0.12 有什么区别？"

**AI 会使用**:
```json
{ "crate_name": "reqwest", "version": "0.11" }
{ "crate_name": "reqwest", "version": "0.12" }
```

## 命令行使用

### 启动服务器

```bash
# 使用默认配置启动服务器（混合模式：HTTP + SSE）
cargo run -- serve

# 使用 Stdio 模式（用于 MCP 客户端集成）
cargo run -- serve --mode stdio

# 使用 HTTP 模式
cargo run -- serve --mode http --host 0.0.0.0 --port 8080

# 启用调试日志
cargo run -- serve --debug

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

> 注意：当前 [`health`](src/main.rs:599) CLI 子命令仍是占位实现，主要用于命令结构演示；真正的健康检查能力目前在 MCP 工具 [`health_check`](src/tools/health.rs:12) 中实现。

## 配置

### 配置文件示例

创建 `config.toml`：

```toml
[server]
name = "crates-docs"
version = "0.1.5"
description = "高性能 Rust crate 文档查询 MCP 服务器"
host = "0.0.0.0"
port = 8080
transport_mode = "hybrid"
enable_sse = true
enable_oauth = false
max_connections = 100
request_timeout_secs = 30
response_timeout_secs = 60
allowed_hosts = ["localhost", "127.0.0.1", "0.0.0.0"]
allowed_origins = ["http://localhost:*"]

[cache]
cache_type = "memory"  # 或 "redis"
memory_size = 1000
# redis_url = "redis://localhost:6379"
default_ttl = 3600

[oauth]
enabled = false
# client_id = "your-client-id"
# client_secret = "your-client-secret"
# redirect_uri = "http://localhost:8080/oauth/callback"
# authorization_endpoint = "https://provider.com/oauth/authorize"
# token_endpoint = "https://provider.com/oauth/token"
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

## 传输协议

### Stdio 模式（推荐用于 MCP 客户端）

最简单、最安全的模式，适合与 MCP 客户端（Claude Desktop、Cursor 等）集成：

```bash
cargo run -- serve --mode stdio
```

### HTTP 模式（Streamable HTTP）

适合网络服务和需要远程访问的场景：

```bash
cargo run -- serve --mode http --host 0.0.0.0 --port 8080
```

### SSE 模式（Server-Sent Events）

用于向后兼容，推荐使用 Hybrid 模式：

```bash
cargo run -- serve --mode sse --host 0.0.0.0 --port 8080
```

### 混合模式（推荐用于网络服务）

同时支持 HTTP 和 SSE，最灵活的模式：

```bash
cargo run -- serve --mode hybrid --host 0.0.0.0 --port 8080
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

# 运行所有特性测试
cargo test --all-features
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

```yaml
version: '3.8'
services:
  crates-docs:
    build: .
    ports:
      - "8080:8080"
    environment:
      CRATES_DOCS_HOST: 0.0.0.0
      CRATES_DOCS_PORT: 8080
      CRATES_DOCS_TRANSPORT_MODE: hybrid
    volumes:
      - ./config.toml:/app/config.toml:ro
      - ./logs:/app/logs
      - ./data:/app/data

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

启动服务：

```bash
docker compose up -d
```

## API 端点

### 健康检查

当前 README 中曾使用 `GET /health` 作为示意，但从现有代码看，项目已明确实现并可确认的网络端点主要是 MCP 传输端点，而不是独立的 HTTP 健康检查路由。也就是说：

- CLI 健康检查命令见 [`health_command()`](src/main.rs:599)
- MCP 工具健康检查实现见 [`health_check`](src/tools/health.rs:12)
- HTTP/SSE 服务端点由传输层启动，见 [`run_http_server()`](src/server/transport.rs:47)、[`run_sse_server()`](src/server/transport.rs:90)、[`run_hybrid_server()`](src/server/transport.rs:133)

因此，这里更准确的 API 描述应聚焦 MCP 端点：

### MCP 端点

- `POST /mcp` - MCP Streamable HTTP 端点
- `GET /sse` - MCP SSE 端点

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

调整配置文件中的缓存大小：

```toml
[cache]
memory_size = 500
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