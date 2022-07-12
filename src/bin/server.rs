//! mini-redis server
//! This file is the entry point for the server implemented in the library.
//! It performs command line parsing and passes the arguments on to `mini_redis::server`.

use clap::Parser;
use mini_redis::{server, DEFAULT_PORT};
#[cfg(feature = "otel")]
use opentelemetry::{global, sdk::trace as sdktrace};
#[cfg(feature = "otel")]
use opentelemetry_aws::trace::XrayPropagator;
use tokio::{net::TcpListener, signal};
#[cfg(feature = "otel")]
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
    EnvFilter,
};

#[tokio::main]
async fn main() -> mini_redis::Result<()> {
    set_up_logging()?;

    let cli = Cli::parse();
    let port = cli.port.unwrap_or(DEFAULT_PORT);

    // Build a TCP listener.
    let listener = TcpListener::bind(&format!("127.0.0.1:{}", port)).await?;

    server::run(listener, signal::ctrl_c()).await;

    Ok(())
}

/// A redis server.
#[derive(Parser, Debug)]
#[clap(author, version, about = "A Redis server")]
struct Cli {
    /// Port.
    #[clap(short, long)]
    port: Option<u16>,
}

#[cfg(not(feature = "otel"))]
fn set_up_logging() -> mini_redis::Result<()> {
    tracing_subscriber::fmt::try_init()
}

#[cfg(feature = "otel")]
fn set_up_logging() -> Result<(), TryInitError> {
    // Set the global propagator to X-Ray propagator
    global::set_text_map_propagator(XrayPropagator::default());

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(
            sdktrace::config()
                .with_sampler(sdktrace::Sampler::AlwaysOn)
                // Needed in order to convert the trace IDs into an Xray-compatible format
                .with_id_generator(sdktrace::XrayIdGenerator::default()),
        )
        .install_simple()
        .expect("Unable to initialize OtlpPipeline");

    // Create a tracing layer with the configured tracer.
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Parse an `EnvFilter` configuration from the `RUST_LOG` environment variable.
    let filter = EnvFilter::from_default_env();

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`.
    tracing_subscriber::registry()
        .with(opentelemetry)
        .with(filter)
        .with(fmt::Layer::default())
        .try_init()
}
