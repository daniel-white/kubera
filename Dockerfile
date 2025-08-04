ARG RUST_VERSION=1.88
ARG DEBIAN_RELEASE=bookworm
ARG RUST_CONFIGURATION=release
ARG RUST_PROFILE=release

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/kubera

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG RUST_CONFIGURATION
ARG RUST_PROFILE
RUN apt update && apt install -y cmake

# Copy recipe and build dependencies
COPY --from=planner /usr/src/kubera/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json $(if [ "${RUST_CONFIGURATION}" = "release" ]; then echo "--release"; fi)

# Build application
COPY . .
RUN cargo build $(if [ "${RUST_CONFIGURATION}" = "release" ]; then echo "--release"; fi) --profile ${RUST_PROFILE} --bins

FROM debian:${DEBIAN_RELEASE}-slim AS runtime
ARG RUST_CONFIGURATION
RUN apt update && apt install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/kubera/target/${RUST_CONFIGURATION}/kubera_control_plane /usr/local/bin/
COPY --from=builder /usr/src/kubera/target/${RUST_CONFIGURATION}/kubera_gateway /usr/local/bin/

# Add non-root user for security
RUN useradd kubera
USER kubera