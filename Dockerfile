# === BUILD STAGE ===
FROM rust:1.88-alpine AS builder

# Install build dependencies including protoc
RUN apk add --no-cache \
    musl-dev \
    capnproto \
    protobuf-dev \
    protoc

WORKDIR /log-server

# Copy source code
COPY . .

# Build the application
RUN cargo build --release

# === RUNTIME STAGE ===
FROM alpine:3.21

# Install runtime dependencies including protoc
RUN apk add --no-cache \
    ca-certificates \
    tzdata 

WORKDIR /log-server

# Copy the binary from the build stage
COPY --from=builder /log-server/target/release/log_server /log-server/log-server

# Make binary executable
RUN chmod +x /log-server/log-server

# Set the entrypoint
ENTRYPOINT ["/log-server/log-server"]