#!/bin/bash
# Definition of Done gate: check, format, clippy, test.
# Run before marking any step complete. See plan §14.2.
#
# Usage: ./scripts/gate.sh

set -e

cargo check --workspace && \
  cargo fmt --all && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  cargo test --workspace --no-fail-fast
