# 使用 Rust 官方镜像作为构建环境
FROM rust:1.75-slim AS builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 创建工作目录
WORKDIR /app

# 复制 Cargo 文件
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY build.rs ./

# 构建项目
RUN cargo build --release

# 使用轻量级运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN useradd -m -u 1000 -s /bin/bash appuser

# 创建工作目录
WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/crates-docs /app/crates-docs
COPY --from=builder /app/target/release/build/*/build_script_build-*/out/*.so /app/ 2>/dev/null || true

# 复制配置文件示例
COPY examples/config.example.toml /app/config.toml

# 创建日志目录
RUN mkdir -p /app/logs && chown -R appuser:appuser /app

# 切换到非 root 用户
USER appuser

# 暴露端口
EXPOSE 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# 设置环境变量
ENV RUST_LOG=info
ENV CRATES_DOCS_HOST=0.0.0.0
ENV CRATES_DOCS_PORT=8080
ENV CRATES_DOCS_TRANSPORT_MODE=hybrid

# 启动命令
CMD ["/app/crates-docs", "serve", "--config", "/app/config.toml"]