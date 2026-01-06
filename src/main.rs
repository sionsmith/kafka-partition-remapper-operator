//! OSO Kafka Partition Remapper Kubernetes Operator
//!
//! Main entry point for the operator. Sets up the Kubernetes client,
//! registers CRD controllers, and runs the reconciliation loops.

use kube::Client;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use kafka_partition_remapper_operator::{
    controllers::{remapper_controller, Context},
    metrics,
};

/// Default metrics port
const METRICS_PORT: u16 = 8080;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    init_tracing();

    info!("Starting OSO Kafka Partition Remapper Operator");

    // Create Kubernetes client
    let client = Client::try_default().await?;
    info!("Connected to Kubernetes API server");

    // Create shared context
    let context = Context::new(client.clone());

    // Start metrics server
    let metrics_handle = tokio::spawn(metrics::serve(METRICS_PORT));
    info!("Metrics server starting on port {}", METRICS_PORT);

    // Run the remapper controller
    let controller_handle = tokio::spawn(remapper_controller::run(context));

    // Handle graceful shutdown
    tokio::select! {
        _ = controller_handle => {
            error!("Remapper controller exited unexpectedly");
        }
        _ = metrics_handle => {
            error!("Metrics server exited unexpectedly");
        }
        _ = shutdown_signal() => {
            info!("Received shutdown signal, stopping operator");
        }
    }

    info!("OSO Kafka Partition Remapper Operator stopped");
    Ok(())
}

/// Initialize tracing subscriber
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,kafka_partition_remapper_operator=debug,kube=warn,hyper=warn")
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

/// Wait for shutdown signal (SIGTERM or SIGINT)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received CTRL+C signal");
        }
        _ = terminate => {
            info!("Received SIGTERM signal");
        }
    }
}
