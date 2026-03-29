# Dockerfile for local development/testing
# Uses distroless for production-like testing locally

FROM rust:1.88 AS builder

WORKDIR /app

# Copy source
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY src ./src

# Build
RUN cargo build --release --bin crates-docs

# Runtime - distroless (smaller than full Debian)
FROM gcr.io/distroless/cc-debian12:latest

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/crates-docs /app/crates-docs

# Copy config
COPY examples/config.example.toml /app/config.toml

EXPOSE 8080

ENV RUST_LOG=info
ENV CRATES_DOCS_HOST=0.0.0.0
ENV CRATES_DOCS_PORT=8080
ENV CRATES_DOCS_TRANSPORT_MODE=hybrid

USER 65534:65534

ENTRYPOINT ["/app/crates-docs"]
CMD ["serve", "--config", "/app/config.toml"]
