ARG RUST_VERSION=1.86
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
RUN cargo install cargo-watch
WORKDIR /usr/src/kubera
COPY Cargo.toml Cargo.lock ./
COPY controller/Cargo.toml ./
COPY core/Cargo.toml ./
RUN cargo fetch
COPY . .
RUN cargo build --release --bin kubera-controller

FROM debian:${DEBIAN_RELEASE}-slim
COPY --from=builder /usr/src/kubera/target/release/kubera_controller /usr/local/bin/kubera-controller
CMD ["kubera-controller"]