#!/bin/bash
# Run coverage with cargo-llvm-cov. Enforces >= 85% **line** coverage on the
# measured scope (see README: excluded paths are thin I/O glue or integration-heavy).
#
# Usage: ./scripts/coverage.sh

set -euo pipefail

EXCLUDE='(network/tcp|client/grpc\.rs|prometheus\.rs|tracing\.rs|graph/builder\.rs|client/mod\.rs|cli_handlers\.rs|launcher\.rs)'

exec cargo llvm-cov --all-features --workspace --tests --no-clean \
  --ignore-filename-regex="$EXCLUDE" \
  --summary-only \
  --fail-under-lines 85
