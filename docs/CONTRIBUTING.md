# Contributing to FlowRaft

## Development Setup

### Prerequisites

- **Rust** 1.93 or later (`rustup` recommended)
- **Protocol Buffers** compiler (`protoc`) for `flow-raft-proto`

```bash
# Clone the repository
git clone https://github.com/vvylym/flow-raft
cd flow-raft

# Build
cargo build

# Run tests
cargo test
```

### Code Quality

Before submitting changes, run:

```bash
cargo check
cargo fmt
cargo clippy --all-targets
cargo test
```

Coverage is enforced (e.g. via CI). Run locally with:

```bash
./scripts/coverage.sh
# or: cargo llvm-cov --workspace --tests
```

## Pull Requests

1. **Branch** from `main` and keep changes focused.
2. **Test**: ensure all tests pass and coverage does not decrease.
3. **Format**: run `cargo fmt` and `cargo clippy`.
4. **Docs**: update user-facing docs if you change behavior or APIs.
5. **Description**: clearly describe the change and, if applicable, link an issue.

## Project Structure

- `flow-raft` – main facade and examples
- `flow-raft-core` – workflow/task state machines, DAG utilities
- `flow-raft-raft` – Raft consensus and replication
- `flow-raft-api` – graph builder, gRPC client, workflow definitions
- `flow-raft-server` – gRPC service, cluster, handlers
- `flow-raft-observability` – metrics, history, watcher
- `flow-raft-proto` – protobuf and gRPC definitions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
