ARG RUST_VERSION=1.87
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
#RUN cargo install cargo-watch
WORKDIR /usr/src/kubera
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY core/Cargo.toml ./core/
COPY controlplane/Cargo.toml ./controlplane/
# TODO do a cargo fetch here
COPY . .
RUN cargo build --release --bin kubera_controlplane

FROM debian:${DEBIAN_RELEASE}-slim
COPY --from=builder /usr/src/kubera/target/release/kubera_controlplane /usr/local/bin/kubera-controlplane
CMD ["kubera-controlplane"]