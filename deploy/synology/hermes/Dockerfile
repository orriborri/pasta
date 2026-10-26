# Operator must supply a tested upstream image pinned by digest.
ARG HERMES_IMAGE
FROM rust:1.91-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --locked --release -p pasta-assistant -p kb-cli -p kb-mcp

FROM ${HERMES_IMAGE}
COPY --from=build /app/target/release/pasta-assistant /app/target/release/kb /app/target/release/kb-mcp /usr/local/bin/
# Keep upstream /init entrypoint and its privilege drop.
