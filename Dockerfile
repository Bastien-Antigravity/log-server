# === BUILD STAGE ===
FROM rust:1.85-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev capnproto protobuf-dev

WORKDIR /log-server

# Copy source code
COPY . .

# Build the application
RUN cargo build --release

# === RUNTIME STAGE ===
FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache ca-certificates tzdata

WORKDIR /log-server

# Copy the binary from the build stage
COPY --from=builder /log-server/target/release/log_server /log-server/log-server

# Expose port (default 9020)
EXPOSE 9020

# Set the entrypoint
ENTRYPOINT ["/log-server/log-server"]
