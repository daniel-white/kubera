ARG RUST_VERSION=1.86
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
RUN cargo install cargo-watch
WORKDIR /usr/src/kubera
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml ./
COPY gateway/Cargo.toml ./
RUN cargo fetch
COPY . .
RUN cargo build --release --bin kubera_gateway

FROM debian:${DEBIAN_RELEASE}-slim
COPY --from=builder /usr/src/kubera/target/release/kubera_gateway /usr/local/bin/kubera_gateway
CMD ["kubera_gateway"]