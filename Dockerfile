# syntax=docker/dockerfile:1

# Build stage
FROM rust:1.70-alpine AS builder

WORKDIR /app

# Install musl-dev for static linking
RUN apk add --no-cache musl-dev

# Copy dependency files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy source code
COPY src ./src

# Build for musl target
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release

# Runtime stage - use scratch for minimal image
FROM scratch

# Copy the statically linked binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/url-proxy /url-proxy

EXPOSE 3000

ENTRYPOINT ["/url-proxy"]