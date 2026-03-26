# === BUILD ===
FROM rust:1.85-alpine AS builder
RUN apk add --no-cache musl-dev capnproto protobuf-dev
WORKDIR /app
COPY . .
RUN cargo build --release

# === RUNTIME ===
FROM alpine:3.21
RUN apk add --no-cache ca-certificates tzdata
WORKDIR /app
COPY --from=builder /app/target/release/log_server /server
EXPOSE 9020
ENTRYPOINT ["/server"]
