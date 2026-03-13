# Crates Docs MCP 服务器

[![Crates.io](https://img.shields.io/crates/v/crates-docs.svg)](https://crates.io/crates/crates-docs)
[![Documentation](https://docs.rs/crates-docs/badge.svg)](https://docs.rs/crates-docs)
[![Docker](https://img.shields.io/docker/v/kingingwang/crates-docs/latest?label=docker)](https://hub.docker.com/r/kingingwang/crates-docs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/KingingWang/crates-docs/workflows/CI/badge.svg)](https://github.com/KingingWang/crates-docs/actions)
[![codecov](https://codecov.io/gh/kingingwang/crates-docs/branch/main/graph/badge.svg)](https://codecov.io/gh/kingingwang/crates-docs)

一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议。

## 特性

- 🚀 **高性能**: 异步 Rust + LRU/TTL 内存缓存，可选 Redis 扩展
- 📦 **多架构 Docker 镜像**: 支持 `linux/amd64` 和 `linux/arm64`
- 🔧 **多种传输协议**: Stdio、HTTP (Streamable HTTP)、SSE、Hybrid
- 📚 **完整文档查询**: crate 搜索、文档查找、特定项目查询
- 🛡️ **安全可靠**: 速率限制、连接池、请求验证
- 📊 **健康监控**: 内置健康检查和性能监控
- 🏗️ **模块化架构**: 清晰的模块划分，易于扩展和维护

## 项目结构

```
src/
├── lib.rs              # 库入口，导出公共 API
├── main.rs             # 程序入口
├── cache/              # 缓存层
│   ├── mod.rs          # Cache trait 定义
│   ├── memory.rs       # 内存缓存实现（LRU + TTL）
│   └── redis.rs        # Redis 缓存实现
├── cli/                # 命令行接口
│   ├── mod.rs          # CLI 定义和路由
│   ├── commands.rs     # 子命令定义
│   ├── serve_cmd.rs    # serve 命令实现
│   ├── test_cmd.rs     # test 命令实现
│   ├── config_cmd.rs   # config 命令实现
│   ├── health_cmd.rs   # health 命令实现
│   └── version_cmd.rs  # version 命令实现
├── config/             # 配置管理
│   └── mod.rs          # 配置结构和加载逻辑
├── error/              # 错误处理
│   └── mod.rs          # 错误类型定义
├── server/             # 服务器核心
│   ├── mod.rs          # 服务器定义
│   ├── auth.rs         # OAuth 认证
│   ├── handler.rs      # MCP 请求处理
│   └── transport.rs    # 传输层实现
├── tools/              # MCP 工具
│   ├── mod.rs          # 工具注册表
│   ├── health.rs       # 健康检查工具
│   └── docs/           # 文档查询工具
│       ├── mod.rs      # 文档服务
│       ├── cache.rs    # 文档缓存
│       ├── html.rs     # HTML 处理
│       ├── lookup_crate.rs  # crate 文档查找
│       ├── lookup_item.rs   # 项目文档查找
│       └── search.rs        # crate 搜索
└── utils/              # 工具函数
    └── mod.rs          # 通用工具
```

## 快速开始

### 使用 Docker（推荐）

```bash
# 从 Docker Hub 拉取镜像
docker pull kingingwang/crates-docs:latest

# 运行容器（官方镜像内置配置默认监听 0.0.0.0:8080）
docker run -d --name crates-docs -p 8080:8080 kingingwang/crates-docs:latest

# 使用自定义配置
docker run -d --name crates-docs -p 8080:8080 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  kingingwang/crates-docs:latest
```

### Docker Compose

```yaml
version: '3.8'
services:
  crates-docs:
    image: kingingwang/crates-docs:latest
    ports:
      - "8080:8080"
    environment:
      CRATES_DOCS_HOST: 0.0.0.0
      CRATES_DOCS_PORT: 8080
      CRATES_DOCS_TRANSPORT_MODE: hybrid
    volumes:
      - ./config.toml:/app/config.toml:ro
      - ./logs:/app/logs
    restart: unless-stopped
```

```bash
docker compose up -d
```

### 从源码构建

```bash
git clone https://github.com/KingingWang/crates-docs.git
cd crates-docs
cargo build --release
./target/release/crates-docs serve
```

### 从 crates.io 安装

```bash
cargo install crates-docs
crates-docs serve
```

## MCP 客户端集成

### Claude Desktop

编辑配置文件：
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

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

### Cursor

编辑 `~/.cursor/mcp.json`：

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

### Windsurf

编辑 `~/.codeium/windsurf/mcp_config.json`：

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

### Cherry Studio

1. 打开 Cherry Studio 设置
2. 找到 `MCP 服务器` 选项
3. 点击 `添加服务器`
4. 填写参数：

| 字段 | 值 |
|------|------|
| 名称 | `crates-docs` |
| 类型 | `STDIO` |
| 命令 | `/path/to/crates-docs` |
| 参数1 | `serve` |
| 参数2 | `--mode` |
| 参数3 | `stdio` |

5. 点击保存

> **注意**：将 `/path/to/crates-docs` 替换为实际的可执行文件路径。

### HTTP 模式

适合远程访问或网络服务：

```bash
crates-docs serve --mode hybrid --host 0.0.0.0 --port 8080
```

客户端配置：

```json
{
  "mcpServers": {
    "crates-docs": {
      "url": "http://your-server:8080/mcp"
    }
  }
}
```

## MCP 工具

### 1. lookup_crate - 查找 Crate 文档

从 docs.rs 获取完整文档。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `crate_name` | string | ✅ | Crate 名称，如 `serde`、`tokio` |
| `version` | string | ❌ | 版本号，默认最新 |
| `format` | string | ❌ | 输出格式：`markdown`（默认）、`text`、`html` |

```json
{ "crate_name": "serde" }
{ "crate_name": "tokio", "version": "1.35.0" }
```

### 2. search_crates - 搜索 Crate

从 crates.io 搜索 Rust crate。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `query` | string | ✅ | 搜索关键词 |
| `limit` | number | ❌ | 结果数量（1-100），默认 10 |
| `format` | string | ❌ | 输出格式：`markdown`、`text`、`json` |

```json
{ "query": "web framework", "limit": 5 }
```

### 3. lookup_item - 查找特定项目

查找 crate 中的特定类型、函数或模块。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `crate_name` | string | ✅ | Crate 名称 |
| `item_path` | string | ✅ | 项目路径，如 `serde::Serialize` |
| `version` | string | ❌ | 版本号 |
| `format` | string | ❌ | 输出格式 |

```json
{ "crate_name": "serde", "item_path": "serde::Serialize" }
{ "crate_name": "tokio", "item_path": "tokio::runtime::Runtime" }
```

### 4. health_check - 健康检查

检查服务器和外部服务状态。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `check_type` | string | ❌ | `all`、`external`、`internal`、`docs_rs`、`crates_io` |
| `verbose` | boolean | ❌ | 详细输出 |

```json
{ "check_type": "all", "verbose": true }
```

## 使用示例

### 了解新 crate

**用户**: "帮我了解一下 serde"

**AI 调用**: `{ "crate_name": "serde" }`

### 查找特定功能

**用户**: "tokio 怎么创建异步任务？"

**AI 调用**: `{ "crate_name": "tokio", "item_path": "tokio::spawn" }`

### 搜索相关 crate

**用户**: "有什么好用的 HTTP 客户端？"

**AI 调用**: `{ "query": "http client", "limit": 10 }`

## 命令行

```bash
# 启动服务器
crates-docs serve                          # 混合模式
crates-docs serve --mode stdio             # Stdio 模式
crates-docs serve --mode http --port 8080  # HTTP 模式

# 生成配置
crates-docs config --output config.toml
crates-docs config --output config.toml --force

# 测试工具
crates-docs test --tool lookup_crate --crate-name serde
crates-docs test --tool search_crates --query "async"

# CLI 健康检查入口
crates-docs health
crates-docs health --check-type external --verbose

# 版本信息
crates-docs version
```

> 全局参数见 [`Cli`](src/cli/mod.rs:27)，常用项包括 `--config`、`--debug`、`--verbose`。
>
> 当前 [`run_health_command()`](src/cli/health_cmd.rs:4) 仍是 CLI 级占位输出；需要真实探测 docs.rs / crates.io 状态时，应优先使用 MCP 工具 [`health_check`](src/tools/health.rs:11)。

## 配置

### 配置文件

下面示例展示的是常见网络部署配置；使用 [`run_config_command()`](src/cli/config_cmd.rs:6) 生成文件时，实际内容来自 [`AppConfig::default()`](src/config/mod.rs:11)，默认监听地址仍是 `127.0.0.1`。

```toml
[server]
name = "crates-docs"
host = "0.0.0.0"
port = 8080
transport_mode = "hybrid"
allowed_hosts = ["localhost", "127.0.0.1"]
allowed_origins = ["http://localhost:*"]

[cache]
cache_type = "memory"
memory_size = 1000
default_ttl = 3600

[logging]
level = "info"
enable_console = true
enable_file = false  # 默认仅控制台输出

[performance]
http_client_pool_size = 10
cache_max_size = 1000
enable_response_compression = true
```

> **启用文件日志**：设置 `enable_file = true` 并配置 `file_path` 可写入日志文件。
>
> **默认监听地址说明**：直接使用二进制且未提供配置文件时，[`ServerConfig::default()`](src/config/mod.rs:143) 的默认 `host` 是 `127.0.0.1`；官方 Docker 镜像通过内置配置和环境变量使用 `0.0.0.0`，便于容器对外提供服务。

### 环境变量

```bash
export CRATES_DOCS_NAME="crates-docs"
export CRATES_DOCS_HOST="0.0.0.0"
export CRATES_DOCS_PORT="8080"
export CRATES_DOCS_TRANSPORT_MODE="hybrid"

# 日志配置
export CRATES_DOCS_LOG_LEVEL="info"
export CRATES_DOCS_ENABLE_CONSOLE="true"
export CRATES_DOCS_ENABLE_FILE="true"
```

> [`AppConfig::from_env()`](src/config/mod.rs:304) 当前支持的环境变量包括 `CRATES_DOCS_NAME`、`CRATES_DOCS_HOST`、`CRATES_DOCS_PORT`、`CRATES_DOCS_TRANSPORT_MODE`、`CRATES_DOCS_LOG_LEVEL`、`CRATES_DOCS_ENABLE_CONSOLE`、`CRATES_DOCS_ENABLE_FILE`。
>
> **文件日志**：默认禁用。设置 `CRATES_DOCS_ENABLE_FILE=true` 后，日志仍写入配置中的 `file_path`（默认 `./logs/crates-docs.log`）；当前不支持通过环境变量覆盖 `file_path`。

## 传输协议

| 模式 | 适用场景 | 端点 |
|------|---------|------|
| `stdio` | MCP 客户端集成（推荐） | 标准输入输出 |
| `http` | 网络服务 | `POST /mcp` |
| `sse` | 向后兼容 | `GET /sse` |
| `hybrid` | 网络服务（推荐） | `/mcp` + `/sse` |

## MCP 端点

- `POST /mcp` - MCP Streamable HTTP 端点
- `GET /sse` - MCP SSE 端点

> 注意：这些是 MCP 协议端点，不是普通的 HTTP API。需要使用 MCP 客户端进行交互。

## 缓存策略

### 内存缓存（默认）

- 当前实现位于 [`MemoryCache`](src/cache/memory.rs:29)
- 使用 LRU 淘汰策略
- 支持 TTL 过期
- 适用于单实例部署

### Redis 缓存

- 支持分布式部署
- 支持持久化
- 通过 feature flag 启用：`cache-redis`

```bash
cargo build --release --features cache-redis
```

配置示例：

```toml
[cache]
cache_type = "redis"
redis_url = "redis://localhost:6379"
default_ttl = 3600
```

## 部署

### Docker

```bash
# 使用预构建镜像
docker pull kingingwang/crates-docs:latest
docker run -d -p 8080:8080 kingingwang/crates-docs:latest

# 或使用特定版本
docker pull kingingwang/crates-docs:0.3.0
```

### Systemd

创建 `/etc/systemd/system/crates-docs.service`：

```ini
[Unit]
Description=Crates Docs MCP Server
After=network.target

[Service]
Type=simple
User=crates-docs
WorkingDirectory=/opt/crates-docs
ExecStart=/opt/crates-docs/crates-docs serve --config /etc/crates-docs/config.toml
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable crates-docs
sudo systemctl start crates-docs
```

## 开发

```bash
# 构建
cargo build --release

# 运行所有测试
cargo test --all-features

# 运行 clippy 检查
cargo clippy --all-features --all-targets -- -D warnings

# 格式化检查
cargo fmt --check

# 运行完整 CI 流程
cargo clippy --all-features --all-targets -- -D warnings && \
cargo test --all-features && \
cargo fmt --check
```

### Feature Flags

| Feature | 描述 |
|---------|------|
| `default` | 默认启用：`server`、`stdio`、`macros`、`cache-memory`、`logging` |
| `server` | 启用 rust-mcp-sdk 服务端能力 |
| `client` | 启用 rust-mcp-sdk 客户端能力 |
| `stdio` | 启用 Stdio 传输 |
| `hyper-server` | 启用 HTTP 服务器 |
| `streamable-http` | 启用 Streamable HTTP |
| `sse` | 启用 SSE 传输 |
| `macros` | 启用 MCP 宏支持 |
| `auth` | 启用 OAuth 认证支持 |
| `cache-memory` | 启用内存缓存相关支持 |
| `cache-redis` | 启用 Redis 缓存 |
| `tls` | 启用 TLS/SSL 支持 |
| `logging` | 启用日志相关支持 |

## 故障排除

### 端口被占用

```bash
lsof -i :8080
kill -9 <PID>
```

### 网络问题

```bash
curl -I https://docs.rs/
curl -I https://crates.io/
```

### 日志

启用文件日志时，默认日志文件路径为 `./logs/crates-docs.log`。

```toml
[logging]
level = "debug"
```

## 许可证

MIT License

## 贡献

欢迎 Issue 和 Pull Request！

1. Fork 仓库
2. 创建分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### 开发指南

- 所有代码必须通过 `cargo clippy --all-features --all-targets -- -D warnings`
- 所有测试必须通过 `cargo test --all-features`
- 新功能需要添加相应的单元测试
- 遵循现有的代码风格和文档规范

## 致谢

- [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) - MCP SDK
- [docs.rs](https://docs.rs) - Rust 文档服务
- [crates.io](https://crates.io) - Rust 包注册表
- [lru](https://crates.io/crates/lru) - 内存缓存淘汰策略实现

## 支持

- [Issues](https://github.com/KingingWang/crates-docs/issues)
- Email: kingingwang@foxmail.com