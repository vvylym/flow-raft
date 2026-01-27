# Multi-stage Dockerfile for flowraft-node and flowraft.
# Build: docker build -t flowraft .
# Run:   docker run ... flowraft-node serve (see docker-compose or env NODE_ID, GRPC_BIND, etc.)

FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /build

# Copy workspace and member crates (flow-raft-server and its path deps; full workspace for resolve)
COPY Cargo.toml Cargo.lock ./
COPY flow-raft ./flow-raft
COPY flow-raft-core ./flow-raft-core
COPY flow-raft-testing ./flow-raft-testing
COPY flow-raft-proto ./flow-raft-proto
COPY flow-raft-api ./flow-raft-api
COPY flow-raft-observability ./flow-raft-observability
COPY flow-raft-raft ./flow-raft-raft
COPY flow-raft-server ./flow-raft-server

RUN cargo build --release -p flow-raft --bins

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/flowraft-node /usr/local/bin/
COPY --from=builder /build/target/release/flowraft /usr/local/bin/

# Default: run node. Override with command. Set env: NODE_ID, GRPC_BIND, HTTP_BIND, RAFT_BIND, DATA_PATH, PEERS, BOOTSTRAP.
ENTRYPOINT ["flowraft-node"]
CMD ["serve"]
