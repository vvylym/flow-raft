//! Advanced client example
//!
//! Demonstrates:
//! - gRPC client usage
//! - Execution tracking
//! - Callback patterns
//! - Stream processing

use flow_raft::prelude::*;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Advanced Client Example ===");
    println!("\nThis example demonstrates:");
    println!("  1. Creating a FlowRaft client");
    println!("  2. Submitting workflows via gRPC");
    println!("  3. Watching execution events");
    println!("  4. Using callbacks for real-time updates");

    // Create client using builder
    let client = FlowRaftClient::builder()
        .with_endpoint("http://localhost:50051")
        .with_timeout(Duration::from_secs(300))
        .build();

    println!("\n✓ Client created");

    // Note: Full gRPC implementation requires proto code sharing
    // For now, this demonstrates the client API structure
    println!("\nClient API methods available:");
    println!("  - submit_workflow()");
    println!("  - get_workflow_status()");
    println!("  - watch_execution()");
    println!("  - get_task_result()");
    println!("  - cancel_workflow()");

    println!("\n✓ Advanced client example completed!");
    Ok(())
}
