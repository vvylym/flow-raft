#!/bin/bash
# Comprehensive benchmark runner for FlowRaft
#
# Usage: ./scripts/benchmark_suite.sh
#
# Runs all benchmarks and generates reports

set -e

echo "Running FlowRaft benchmarks..."
cargo bench --bench flow_raft_benchmarks

echo "Running Temporal comparison benchmarks..."
cargo bench --bench temporal_comparison

echo "Running Airflow comparison benchmarks..."
cargo bench --bench airflow_comparison

echo "Running workflow execution benchmarks..."
cargo bench --bench workflow_execution

echo "All benchmarks completed!"
echo ""
echo "To generate flamegraphs, run:"
echo "  ./scripts/generate_flamegraph.sh flow_raft_benchmarks"
echo "  ./scripts/generate_flamegraph.sh temporal_comparison"
echo "  ./scripts/generate_flamegraph.sh airflow_comparison"
echo "  ./scripts/generate_flamegraph.sh workflow_execution"
