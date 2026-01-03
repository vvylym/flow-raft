#!/bin/bash
# Generate flamegraph for FlowRaft benchmarks
#
# Usage: ./scripts/generate_flamegraph.sh [benchmark_name]
#
# This script builds the benchmarks in release mode, runs perf,
# and generates a flamegraph SVG.

set -e

BENCHMARK_NAME="${1:-flow_raft_benchmarks}"
OUTPUT_FILE="flamegraph_${BENCHMARK_NAME}.svg"

echo "Building benchmarks in release mode..."
cargo build --release --benches

echo "Running perf record on ${BENCHMARK_NAME}..."
perf record --call-graph dwarf \
    target/release/deps/${BENCHMARK_NAME}-* \
    --bench

echo "Generating flamegraph..."
perf script | inferno-flamegraph > "${OUTPUT_FILE}"

echo "Flamegraph saved to: ${OUTPUT_FILE}"
echo "To view: open ${OUTPUT_FILE} in a web browser"
