//! Tests for tracing utilities

use flow_raft_observability::{TracingExporter, init_tracing, shutdown_tracing};

#[test]
fn test_tracing_exporter_variants() {
    let otlp = TracingExporter::OTLP;
    let none = TracingExporter::None;
    assert_eq!(otlp, TracingExporter::OTLP);
    assert_eq!(none, TracingExporter::None);
    assert!(otlp != none);
}

#[test]
fn test_shutdown_tracing_noop() {
    // shutdown_tracing is a no-op; just ensure it doesn't panic
    shutdown_tracing();
}

#[test]
fn test_init_tracing_none() {
    // TracingExporter::None only initializes fmt subscriber.
    // try_init() may fail if already initialized by another test; both outcomes are acceptable.
    let result = init_tracing("test-service", TracingExporter::None, None);
    if result.is_ok() {
        shutdown_tracing();
    }
}
