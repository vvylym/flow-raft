#!/bin/bash
# Comprehensive benchmark runner for FlowRaft
#
# Usage: ./scripts/benchmark_suite.sh
#
# Runs all benchmarks and generates reports

set -e

echo "Running registration benchmarks..."
cargo bench --bench registration

echo "Running execution benchmarks..."
cargo bench --bench execution

echo "All benchmarks completed!"
echo ""
echo "To generate flamegraphs, run:"
echo "  ./scripts/generate_flamegraph.sh registration"
echo "  ./scripts/generate_flamegraph.sh execution"
