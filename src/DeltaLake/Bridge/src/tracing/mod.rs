use std::env;
use std::sync::OnceLock;

use opentelemetry::global;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use tracing_opentelemetry::layer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/*
TODO:
- Ensure provider respects OTEL_SDK_DISABLED env var
- Write helper method to define the parent ID for all new spans
  - TBD how that can be set for the child traces, may need to wrap every FFI call in a span, which probably isn't the worst
- Instrument relevant methods (likely everything in table.rs) with the above helper
- Expose config to set endpoint, and whether HTTP or gRPC is used
*/

static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

pub fn init_tracing(endpoint: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    if TRACER_PROVIDER.get().is_some() {
        return Ok(());
    }

    let endpoint = endpoint
        .map(String::from)
        .or_else(|| env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
        .unwrap_or_else(|| "http://localhost:18889".to_string());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    let resource = Resource::builder().with_service_name("delta-rs").build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .with_id_generator(RandomIdGenerator::default())
        .with_sampler(Sampler::AlwaysOn)
        .build();

    global::set_tracer_provider(provider.clone());

    let tracer = global::tracer("delta-rs");
    let telemetry = layer().with_tracer(tracer);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(telemetry)
        .try_init()
        .ok();

    TRACER_PROVIDER.set(provider.clone()).ok();
    Ok(())
}

pub fn shutdown_tracing() -> OTelSdkResult {
    if let Some(provider) = TRACER_PROVIDER.get() {
        provider.shutdown()?;

        Ok(())
    } else {
        panic!("No tracer provider set");
        Ok(())
    }
}