ARG RUST_VERSION=1.88
ARG DEBIAN_RELEASE=bookworm
ARG RUST_CONFIGURATION=release

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder
RUN apt update && apt install -y \
    cmake

WORKDIR /usr/src/kubera

# Dependencies
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY build/src/lib.rs ./api/src/lib.rs
COPY build/Cargo.toml ./build/
COPY build/src/lib.rs ./build/src/lib.rs
COPY control_plane/Cargo.toml ./control_plane/
COPY build/src/lib.rs ./control_plane/src/main.rs
COPY core/Cargo.toml ./core/
COPY build/src/lib.rs ./core/src/lib.rs
COPY gateway/Cargo.toml ./gateway/
COPY build/src/lib.rs ./gateway/src/main.rs
RUN cargo fetch
RUN cargo build --${RUST_CONFIGURATION}
RUN rm -rf \
    ./api/src/lib.rs \
    ./build/src/lib.rs \
    ./control_plane/src/main.rs \
    ./core/src/lib.rs \
    ./gateway/src/main.rs \
    .target/release/kubera_control_plane \
    .target/release/kubera_gateway

# Build binaries
COPY . .
RUN touch api/src/lib.rs && \
    touch build/src/lib.rs && \
    touch control_plane/src/lib.rs && \
    touch core/src/lib.rs && \
    touch gateway/src/lib.rs
RUN cargo build --${RUST_CONFIGURATION}

FROM debian:${DEBIAN_RELEASE}-slim
ARG RUST_CONFIGURATION
RUN apt update && apt install -y \
    ca-certificates

COPY --from=builder /usr/src/kubera/target/${RUST_CONFIGURATION}/kubera_control_plane /usr/local/bin/
COPY --from=builder /usr/src/kubera/target/${RUST_CONFIGURATION}/kubera_gateway /usr/local/bin/
