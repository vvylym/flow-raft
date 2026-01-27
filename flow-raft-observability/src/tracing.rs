//! Distributed tracing for FlowRaft
//!
//! Provides OpenTelemetry integration for distributed tracing.
//!
//! Note: Jaeger can be accessed via OTLP endpoint, so only OTLP exporter is provided.

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;

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

            let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()
                .map_err(|e| format!("Failed to create OTLP span exporter: {}", e))?;

            let resource = Resource::builder_empty()
                .with_attributes([KeyValue::new("service.name", service_name.clone())])
                .build();
            let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                .with_batch_exporter(otlp_exporter)
                .with_resource(resource)
                .build();
            global::set_tracer_provider(tracer_provider);

            tracing_subscriber::fmt()
                .try_init()
                .map_err(|e| format!("Failed to initialize subscriber: {}", e))?;
        }
        TracingExporter::None => {
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
/// all traces are flushed. No-op when using the global tracer provider;
/// consider holding and shutting down the TracerProvider explicitly if you need flush-on-exit.
pub fn shutdown_tracing() {
    // Global provider in 0.31 does not expose shutdown; force_flush is on BatchSpanProcessor.
    // Leaving as no-op. Callers that need flush can hold the SdkTracerProvider and call shutdown.
}
