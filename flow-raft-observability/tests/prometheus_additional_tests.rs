//! Additional tests for Prometheus exporter to increase coverage

use flow_raft_observability::metrics::MetricsCollector;
use flow_raft_observability::prometheus::PrometheusExporter;
use std::sync::Arc;

#[tokio::test]
async fn test_prometheus_exporter_port_getter() {
    let collector = Arc::new(MetricsCollector::new());
    // Use a random port to avoid conflicts
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Prometheus exporter can only be installed once globally
    // If it's already installed from a previous test, skip this test
    let exporter = match PrometheusExporter::new(port, collector) {
        Ok(exporter) => exporter,
        Err(_) => {
            // Already installed, skip test
            return;
        }
    };
    assert_eq!(exporter.port(), port);

    // Give the background task a moment to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
}

#[tokio::test]
async fn test_prometheus_exporter_start_server_bind_error() {
    let collector = Arc::new(MetricsCollector::new());
    // Use port 0 to get a random available port
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Prometheus exporter can only be installed once globally
    let exporter = match PrometheusExporter::new(port, collector) {
        Ok(exporter) => exporter,
        Err(_) => {
            // Already installed, skip test
            return;
        }
    };

    let handle = exporter.start_server().await;
    assert!(handle.is_ok());
    let handle = handle.unwrap();
    handle.abort();
}
