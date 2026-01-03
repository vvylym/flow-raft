//! Prometheus metrics exporter for FlowRaft
//!
//! Provides HTTP endpoint for Prometheus to scrape metrics.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::metrics::MetricsCollector;

/// Prometheus exporter for FlowRaft metrics
pub struct PrometheusExporter {
    port: u16,
    metrics_collector: Arc<MetricsCollector>,
    render_tx: tokio::sync::mpsc::Sender<oneshot::Sender<String>>,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new(
        port: u16,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Install the Prometheus recorder globally and get handle
        let handle = PrometheusBuilder::new().install_recorder().map_err(|e| {
            std::io::Error::other(format!("Failed to install Prometheus exporter: {}", e))
        })?;

        // Create channel for requesting metrics rendering
        let (render_tx, mut render_rx) = tokio::sync::mpsc::channel::<oneshot::Sender<String>>(100);

        // Spawn background task that has the handle and responds to render requests
        tokio::spawn(async move {
            while let Some(response_tx) = render_rx.recv().await {
                let metrics = handle.render();
                let _ = response_tx.send(metrics);
            }
        });

        Ok(Self {
            port,
            metrics_collector,
            render_tx,
        })
    }

    /// Start the Prometheus metrics HTTP server
    ///
    /// This starts an HTTP server that serves metrics at /metrics
    /// and health check endpoints at /health and /ready
    pub async fn start_server(
        self,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = ([0, 0, 0, 0], self.port).into();
        let listener = TcpListener::bind(addr).await?;
        let metrics_collector = self.metrics_collector;
        let render_tx = self.render_tx;

        let server_handle = tokio::spawn(async move {
            println!(
                "Prometheus metrics server listening on http://0.0.0.0:{}",
                listener.local_addr().unwrap().port()
            );

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let io = TokioIo::new(stream);
                        let metrics_collector = metrics_collector.clone();
                        let render_tx = render_tx.clone();

                        tokio::task::spawn(async move {
                            let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                                handle_request(req, metrics_collector.clone(), render_tx.clone())
                            });

                            if let Err(err) =
                                http1::Builder::new().serve_connection(io, service).await
                            {
                                eprintln!("Error serving connection: {:?}", err);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {:?}", e);
                    }
                }
            }
        });

        Ok(server_handle)
    }

    /// Get the metrics port
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Handle HTTP requests
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    _metrics_collector: Arc<MetricsCollector>,
    render_tx: tokio::sync::mpsc::Sender<oneshot::Sender<String>>,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let path = req.uri().path();
    let method = req.method();

    match (method, path) {
        (&Method::GET, "/metrics") => {
            // Request metrics rendering from background task
            let (tx, rx) = oneshot::channel();
            render_tx
                .send(tx)
                .await
                .map_err(|e| format!("Failed to send render request: {}", e))?;

            let metrics = rx
                .await
                .map_err(|e| format!("Failed to receive metrics: {}", e))?;

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain; version=0.0.4")
                .body(Full::new(Bytes::from(metrics)))
                .unwrap())
        }
        (&Method::GET, "/health") => {
            // Health check endpoint
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"healthy"}"#)))
                .unwrap())
        }
        (&Method::GET, "/ready") => {
            // Readiness check endpoint
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"ready"}"#)))
                .unwrap())
        }
        _ => {
            // 404 for unknown paths
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap())
        }
    }
}
