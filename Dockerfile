# === BUILD ===
FROM rust:1.85-alpine AS builder
RUN apk add --no-cache musl-dev capnproto protobuf-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY capnp/ capnp/
COPY proto/ proto/
COPY src/ src/
RUN cargo build --release

# === RUNTIME ===
FROM scratch
COPY --from=builder /app/target/release/log_server /server
EXPOSE 9020
ENTRYPOINT ["/server"]
