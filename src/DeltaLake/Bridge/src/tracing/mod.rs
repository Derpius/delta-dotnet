mod error;

use std::env;
use std::sync::Mutex;

use opentelemetry::global;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use tracing_opentelemetry::layer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::tracing::error::TracingError;

/*
TODO:
- Ensure provider respects OTEL_SDK_DISABLED env var
- Write helper method to define the parent ID for all new spans
  - TBD how that can be set for the child traces, may need to wrap every FFI call in a span, which probably isn't the worst
- Instrument relevant methods (likely everything in table.rs) with the above helper
- Expose config to set endpoint, and whether HTTP or gRPC is used
*/

static TRACER_PROVIDER: Mutex<Option<SdkTracerProvider>> = Mutex::new(None);

pub fn init_tracing(endpoint: Option<&str>) -> Result<(), TracingError> {
    let mut provider = TRACER_PROVIDER.lock()
        .map_err(|err| TracingError::InternalError(err.to_string()))?;
    if provider.is_some() {
        return Err(TracingError::AlreadyInitialized);
    }

    let endpoint = endpoint
        .map(String::from)
        .or_else(|| env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
        .unwrap_or_else(|| "http://localhost:18889".to_string());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|err| TracingError::InternalError(err.to_string()))?;

    let resource = Resource::builder().with_service_name("delta-rs").build();

    let new_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .with_id_generator(RandomIdGenerator::default())
        .with_sampler(Sampler::AlwaysOn)
        .build();

    global::set_tracer_provider(new_provider.clone());

    let tracer = global::tracer("delta-rs");
    let telemetry = layer().with_tracer(tracer);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(telemetry)
        .try_init()
        .ok();

    *provider = Some(new_provider);
    Ok(())
}

pub fn shutdown_tracing() -> Result<(), TracingError> {
    let mut provider = TRACER_PROVIDER.lock()
        .map_err(|err| TracingError::InternalError(err.to_string()))?;

    if let Some(provider_ref) = provider.as_ref() {
        let result = match provider_ref.shutdown() {
            Ok(_) => Ok(()),
            Err(OTelSdkError::AlreadyShutdown) => Ok(()),
            Err(OTelSdkError::Timeout(duration)) => Err(TracingError::Timeout(duration)),
            Err(OTelSdkError::InternalFailure(msg)) => Err(TracingError::InternalError(msg)),
        };

        if result.is_ok() {
            *provider = None;
        }

        result
    } else {
        Ok(())
    }
}