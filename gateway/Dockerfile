ARG RUST_VERSION=1.87
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
RUN apt update && apt install -y \
    cmake
#RUN cargo install cargo-watch
WORKDIR /usr/src/kubera
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY core/Cargo.toml ./core/
COPY gateway/Cargo.toml ./gateway/
# TODO RUN cargo fetch
COPY . .
RUN cargo build --release --bin kubera_gateway

FROM debian:${DEBIAN_RELEASE}-slim
COPY --from=builder /usr/src/kubera/target/release/kubera_gateway /usr/local/bin/
CMD ["kubera_gateway"]