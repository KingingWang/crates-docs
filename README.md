# Crates Docs MCP 服务器

[![Crates.io](https://img.shields.io/crates/v/crates-docs.svg)](https://crates.io/crates/crates-docs)
[![Documentation](https://docs.rs/crates-docs/badge.svg)](https://docs.rs/crates-docs)
[![Docker](https://img.shields.io/docker/v/kingingwang/crates-docs/latest?label=docker)](https://hub.docker.com/r/kingingwang/crates-docs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/KingingWang/crates-docs/workflows/CI/badge.svg)](https://github.com/KingingWang/crates-docs/actions)

一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议。

## 特性

- 🚀 **高性能**: 异步 Rust + LRU 智能缓存
- 📦 **多架构 Docker 镜像**: 支持 `linux/amd64` 和 `linux/arm64`
- 🔧 **多种传输协议**: Stdio、HTTP (Streamable HTTP)、SSE
- 📚 **完整文档查询**: crate 搜索、文档查找、特定项目查询
- 🛡️ **安全可靠**: 速率限制、连接池、请求验证
- 📊 **健康监控**: 内置健康检查和性能监控

## 快速开始

### 使用 Docker（推荐）

```bash
# 从 Docker Hub 拉取镜像
docker pull kingingwang/crates-docs:latest

# 运行容器（默认监听 0.0.0.0:8080）
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

# 测试工具
crates-docs test --tool lookup_crate --crate-name serde
crates-docs test --tool search_crates --query "async"
```

## 配置

### 配置文件

创建 `config.toml`：

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
file_path = "./logs/crates-docs.log"
```

### 环境变量

```bash
export CRATES_DOCS_HOST="0.0.0.0"
export CRATES_DOCS_PORT="8080"
export CRATES_DOCS_TRANSPORT_MODE="hybrid"
```

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

## 部署

### Docker

```bash
# 使用预构建镜像
docker pull kingingwang/crates-docs:latest
docker run -d -p 8080:8080 kingingwang/crates-docs:latest

# 或使用特定版本
docker pull kingingwang/crates-docs:0.1.6
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

# 测试
cargo test --all-features

# 代码检查
cargo clippy -- -D warnings
cargo fmt --check
```

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

日志文件：`./logs/crates-docs.log`

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

## 致谢

- [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) - MCP SDK
- [docs.rs](https://docs.rs) - Rust 文档服务
- [crates.io](https://crates.io) - Rust 包注册表

## 支持

- [Issues](https://github.com/KingingWang/crates-docs/issues)
- Email: kingingwang@foxmail.com