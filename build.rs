//! Build script for FlowRaft
//!
//! Compiles protocol buffer definitions.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&["src/api/grpc/proto/flowraft.proto"], &["src/api/grpc/proto"])?;
    Ok(())
}
