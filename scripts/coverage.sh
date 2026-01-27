#!/bin/bash
# Run coverage with cargo-llvm-cov (>= 85% line coverage)
#
# Usage: ./scripts/coverage.sh
#
# Excludes binary/integration-only and generated glue that is not unit-tested:
#   network/tcp, client/grpc, client/mod, prometheus, tracing, graph/builder

set -e

EXCLUDE='(network/tcp|client/grpc\.rs|prometheus\.rs|tracing\.rs|graph/builder\.rs|client/mod\.rs)'

cargo llvm-cov --all-features --workspace --tests --no-clean \
  --ignore-filename-regex="$EXCLUDE" \
  --summary-only
