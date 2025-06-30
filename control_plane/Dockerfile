ARG RUST_VERSION=1.88
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
#RUN cargo install cargo-watch
WORKDIR /usr/src/kubera
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY core/Cargo.toml ./core/
COPY control_plane/Cargo.toml ./control_plane/
# TODO do a cargo fetch here
COPY . .
RUN cargo build --release --bin kubera_control_plane

FROM debian:${DEBIAN_RELEASE}-slim
COPY --from=builder /usr/src/kubera/target/release/kubera_control_plane /usr/local/bin/
CMD ["kubera_control_plane"]