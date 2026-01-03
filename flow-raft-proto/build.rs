//! Build script for FlowRaft proto crate
//!
//! Compiles protocol buffer definitions using tonic-prost-build.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&["proto/flowraft.proto"], &["proto"])?;
    Ok(())
}
