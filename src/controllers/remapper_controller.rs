//! Controller for KafkaPartitionRemapper resources

use futures::StreamExt;
use kube::{
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, ResourceExt,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument};

use crate::controllers::Context;
use crate::crd::KafkaPartitionRemapper;
use crate::metrics::prometheus::{RECONCILE_DURATION, RECONCILIATIONS, RECONCILIATION_ERRORS};
use crate::reconcilers::remapper;
use crate::Error;

/// Finalizer name for cleanup
pub const FINALIZER: &str = "kafka.oso.sh/remapper-finalizer";

/// Run the remapper controller
pub async fn run(ctx: Arc<Context>) {
    let client = ctx.client.clone();
    let remappers: Api<KafkaPartitionRemapper> = Api::all(client.clone());

    info!("Starting KafkaPartitionRemapper controller");

    Controller::new(remappers, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, ctx)
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("Reconciled {:?}", o),
                Err(e) => error!("Reconcile failed: {:?}", e),
            }
        })
        .await;

    info!("KafkaPartitionRemapper controller stopped");
}

/// Reconcile a KafkaPartitionRemapper resource
#[instrument(skip(remapper, ctx), fields(name = %remapper.name_any(), namespace = remapper.namespace().unwrap_or_default()))]
async fn reconcile(
    remapper: Arc<KafkaPartitionRemapper>,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let start = std::time::Instant::now();
    let ns = remapper.namespace().unwrap_or_default();
    let name = remapper.name_any();

    RECONCILIATIONS
        .with_label_values(&["KafkaPartitionRemapper"])
        .inc();

    let remappers: Api<KafkaPartitionRemapper> = Api::namespaced(ctx.client.clone(), &ns);

    let result = finalizer(&remappers, FINALIZER, remapper, |event| async {
        match event {
            Event::Apply(remapper) => apply(&remapper, &ctx).await,
            Event::Cleanup(remapper) => cleanup(&remapper, &ctx).await,
        }
    })
    .await;

    let duration = start.elapsed().as_secs_f64();
    RECONCILE_DURATION
        .with_label_values(&["KafkaPartitionRemapper"])
        .observe(duration);

    match &result {
        Ok(_) => info!(
            "Successfully reconciled {}/{} in {:.2}s",
            ns, name, duration
        ),
        Err(e) => {
            RECONCILIATION_ERRORS
                .with_label_values(&["KafkaPartitionRemapper"])
                .inc();
            error!("Failed to reconcile {}/{}: {:?}", ns, name, e);
        }
    }

    Ok(result?)
}

/// Apply changes for a KafkaPartitionRemapper
async fn apply(remapper: &KafkaPartitionRemapper, ctx: &Context) -> Result<Action, Error> {
    let ns = remapper.namespace().unwrap_or_default();
    let name = remapper.name_any();

    info!("Applying KafkaPartitionRemapper {}/{}", ns, name);

    // Validate the spec
    remapper::validate(remapper)?;

    // Reconcile ConfigMap
    let config_map_name = remapper::reconcile_config_map(remapper, &ctx.client, &ns).await?;

    // Reconcile Deployment
    let deployment_name =
        remapper::reconcile_deployment(remapper, &ctx.client, &ns, &config_map_name).await?;

    // Reconcile Service
    let service_name = remapper::reconcile_service(remapper, &ctx.client, &ns).await?;

    // Update status
    remapper::update_status(
        remapper,
        &ctx.client,
        &ns,
        &config_map_name,
        &deployment_name,
        &service_name,
    )
    .await?;

    // Requeue after 30 seconds to check deployment status
    Ok(Action::requeue(Duration::from_secs(30)))
}

/// Cleanup resources when a KafkaPartitionRemapper is deleted
async fn cleanup(remapper: &KafkaPartitionRemapper, _ctx: &Context) -> Result<Action, Error> {
    let ns = remapper.namespace().unwrap_or_default();
    let name = remapper.name_any();

    info!("Cleaning up KafkaPartitionRemapper {}/{}", ns, name);

    // Resources are cleaned up automatically via owner references
    // Just log and return

    Ok(Action::await_change())
}

/// Error policy for the controller
fn error_policy(remapper: Arc<KafkaPartitionRemapper>, err: &Error, _ctx: Arc<Context>) -> Action {
    let ns = remapper.namespace().unwrap_or_default();
    let name = remapper.name_any();

    error!("Reconciliation error for {}/{}: {:?}", ns, name, err);

    // Requeue with exponential backoff based on error type
    match err {
        Error::KubeError(_) => Action::requeue(Duration::from_secs(30)),
        Error::ConfigError(_) | Error::ValidationError(_) => {
            Action::requeue(Duration::from_secs(300))
        }
        _ => Action::requeue(Duration::from_secs(60)),
    }
}
