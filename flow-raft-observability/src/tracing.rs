//! Distributed tracing for FlowRaft
//!
//! Provides OpenTelemetry integration for distributed tracing.
//!
//! Note: Jaeger can be accessed via OTLP endpoint, so only OTLP exporter is provided.

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TraceError;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use tracing_subscriber::util::SubscriberInitExt;

/// Tracing exporter type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracingExporter {
    /// OTLP exporter (supports OTLP-compatible backends including Jaeger)
    OTLP,
    /// No exporter (console only)
    None,
}

/// Initialize distributed tracing
///
/// # Arguments
/// * `service_name` - Name of the service
/// * `exporter` - Type of exporter to use
/// * `endpoint` - Endpoint for the exporter (e.g., "http://localhost:4317" for OTLP)
///
/// # Returns
/// Ok(()) if initialization succeeded, error otherwise
pub fn init_tracing(
    service_name: impl Into<String>,
    exporter: TracingExporter,
    endpoint: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service_name = service_name.into();

    match exporter {
        TracingExporter::OTLP => {
            let endpoint = endpoint.unwrap_or_else(|| "http://localhost:4317".to_string());

            // Initialize OTLP tracer provider
            // Note: There's a compatibility issue between tracing-opentelemetry 0.25
            // and tracing-subscriber 0.3 that prevents full integration.
            // For now, we initialize the OTLP exporter but use console logging.
            // Full integration will be completed once version compatibility is resolved.
            let mut exporter = opentelemetry_otlp::new_exporter().tonic();

            // Set endpoint using WithExportConfig trait
            exporter = exporter.with_endpoint(endpoint.clone());

            // Install the tracer provider (this sets up the global provider)
            opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(opentelemetry_sdk::trace::Config::default().with_resource(
                    Resource::new(vec![KeyValue::new("service.name", service_name.clone())]),
                ))
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .map_err(|e: TraceError| format!("Failed to initialize OTLP tracer: {}", e))?;

            // For now, use console logging until tracing-opentelemetry compatibility is resolved
            // The OTLP exporter is initialized and will work for direct OpenTelemetry API usage
            tracing_subscriber::fmt()
                .try_init()
                .map_err(|e| format!("Failed to initialize subscriber: {}", e))?;
        }
        TracingExporter::None => {
            // Just use console logging
            // Use try_init to avoid panic if already initialized
            tracing_subscriber::fmt()
                .try_init()
                .map_err(|e| format!("Failed to initialize subscriber: {}", e))?;
        }
    }

    Ok(())
}

/// Shutdown tracing
///
/// This should be called when the application is shutting down to ensure
/// all traces are flushed.
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}
