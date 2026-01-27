#!/usr/bin/env bash
# Start 3 flowraft-node processes on 127.0.0.1 for local development.
# Node 1 initializes the cluster; nodes 2 and 3 join (empty PEERS).
# Prereq: cargo build -p flow-raft --bin flowraft-node
# Data dirs: data/node1, data/node2, data/node3

set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build -p flow-raft --bin flowraft-node 2>/dev/null || true

BIN="$ROOT/target/debug/flowraft-node"
mkdir -p data/node1 data/node2 data/node3

# Node 1: init cluster with all 3
"$BIN" serve \
  --id 1 \
  --raft 127.0.0.1:5010 \
  --grpc 127.0.0.1:50051 \
  --http 127.0.0.1:9090 \
  --data data/node1 \
  --peers "2=127.0.0.1:5011,3=127.0.0.1:5012" &
N1=$!

# Node 2: joining (PEERS empty)
"$BIN" serve \
  --id 2 \
  --raft 127.0.0.1:5011 \
  --grpc 127.0.0.1:50052 \
  --http 127.0.0.1:9091 \
  --data data/node2 &
N2=$!

# Node 3: joining
"$BIN" serve \
  --id 3 \
  --raft 127.0.0.1:5012 \
  --grpc 127.0.0.1:50053 \
  --http 127.0.0.1:9092 \
  --data data/node3 &
N3=$!

echo "Started nodes: $N1 $N2 $N3 (gRPC 50051–50053, HTTP 9090–9092, Raft 5010–5012)"
echo "Stop: kill $N1 $N2 $N3"

wait
