# flow-raft-proto

Protocol buffer definitions and generated code for the FlowRaft gRPC service.

## Contents

- `proto/flowraft.proto` – service and message definitions
- Generated Rust types and client/server stubs (via `tonic-build` in `build.rs`)

## Usage

Add as a dependency when implementing or calling the FlowRaft gRPC API. The `flow-raft-api` client and `flow-raft-server` service depend on this crate.

## Building

Requires `protoc`. The workspace `cargo build` runs the `build.rs` script and generates code into `OUT_DIR`.

## License

MIT
