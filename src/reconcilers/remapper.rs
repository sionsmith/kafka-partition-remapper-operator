//! Reconciliation logic for KafkaPartitionRemapper resources

use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client, ResourceExt};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tracing::info;

use crate::adapters::{deployment_builder, remapper_config, service_builder};
use crate::crd::{Condition, KafkaPartitionRemapper, KafkaPartitionRemapperStatus};
use crate::{Error, Result};

/// Validate a KafkaPartitionRemapper spec
pub fn validate(remapper: &KafkaPartitionRemapper) -> Result<()> {
    let spec = &remapper.spec;

    // Validate bootstrap servers
    if spec.kafka.bootstrap_servers.is_empty() {
        return Err(Error::ValidationError(
            "kafka.bootstrapServers cannot be empty".to_string(),
        ));
    }

    // Validate mapping
    if spec.mapping.physical_partitions == 0 {
        return Err(Error::ValidationError(
            "mapping.physicalPartitions must be >= 1".to_string(),
        ));
    }

    if spec.mapping.virtual_partitions < spec.mapping.physical_partitions {
        return Err(Error::ValidationError(
            "mapping.virtualPartitions must be >= mapping.physicalPartitions".to_string(),
        ));
    }

    if !spec.mapping.virtual_partitions.is_multiple_of(spec.mapping.physical_partitions) {
        return Err(Error::ValidationError(
            "mapping.virtualPartitions must be evenly divisible by mapping.physicalPartitions"
                .to_string(),
        ));
    }

    // Validate offset range
    let min_offset_range: u64 = 1 << 20; // 2^20
    if spec.mapping.offset_range < min_offset_range {
        return Err(Error::ValidationError(format!(
            "mapping.offsetRange must be >= {} (2^20)",
            min_offset_range
        )));
    }

    // Validate replicas
    if spec.replicas < 0 {
        return Err(Error::ValidationError("replicas must be >= 0".to_string()));
    }

    // Validate security protocol
    let valid_protocols = ["PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL"];
    if !valid_protocols.contains(&spec.kafka.security_protocol.as_str()) {
        return Err(Error::ValidationError(format!(
            "kafka.securityProtocol must be one of: {:?}",
            valid_protocols
        )));
    }

    // Validate that TLS secret is provided for SSL protocols
    if (spec.kafka.security_protocol == "SSL" || spec.kafka.security_protocol == "SASL_SSL")
        && spec.kafka.tls_secret.is_none()
    {
        return Err(Error::ValidationError(
            "kafka.tlsSecret is required when using SSL or SASL_SSL protocol".to_string(),
        ));
    }

    // Validate that SASL secret is provided for SASL protocols
    if (spec.kafka.security_protocol == "SASL_PLAINTEXT"
        || spec.kafka.security_protocol == "SASL_SSL")
        && spec.kafka.sasl_secret.is_none()
    {
        return Err(Error::ValidationError(
            "kafka.saslSecret is required when using SASL_PLAINTEXT or SASL_SSL protocol"
                .to_string(),
        ));
    }

    Ok(())
}

/// Reconcile the ConfigMap for proxy configuration
pub async fn reconcile_config_map(
    remapper: &KafkaPartitionRemapper,
    client: &Client,
    namespace: &str,
) -> Result<String> {
    let name = remapper.name_any();
    let config_map_name = format!("{}-config", name);

    // Determine advertised address
    let advertised_address = remapper
        .spec
        .listen
        .advertised_address
        .clone()
        .unwrap_or_else(|| {
            format!(
                "{}.{}.svc.cluster.local:{}",
                name, namespace, remapper.spec.listen.port
            )
        });

    // Build the proxy configuration YAML
    let config_yaml = remapper_config::build_proxy_config(&remapper.spec, &advertised_address)?;

    // Create ConfigMap
    let mut data = BTreeMap::new();
    data.insert("config.yaml".to_string(), config_yaml);

    let config_map = ConfigMap {
        metadata: ObjectMeta {
            name: Some(config_map_name.clone()),
            namespace: Some(namespace.to_string()),
            owner_references: Some(vec![build_owner_reference(remapper)]),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };

    let config_maps: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    let patch_params = PatchParams::apply("kafka-partition-remapper-operator");

    config_maps
        .patch(&config_map_name, &patch_params, &Patch::Apply(&config_map))
        .await
        .map_err(|e| Error::KubeError(format!("Failed to create/update ConfigMap: {}", e)))?;

    info!("Reconciled ConfigMap {}/{}", namespace, config_map_name);

    Ok(config_map_name)
}

/// Reconcile the Deployment for proxy pods
pub async fn reconcile_deployment(
    remapper: &KafkaPartitionRemapper,
    client: &Client,
    namespace: &str,
    config_map_name: &str,
) -> Result<String> {
    let name = remapper.name_any();

    // Calculate config hash for rolling updates
    let config_hash = calculate_config_hash(remapper);

    // Build Deployment
    let deployment = deployment_builder::build_deployment(remapper, config_map_name, &config_hash);

    let deployments: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let patch_params = PatchParams::apply("kafka-partition-remapper-operator");

    deployments
        .patch(&name, &patch_params, &Patch::Apply(&deployment))
        .await
        .map_err(|e| Error::KubeError(format!("Failed to create/update Deployment: {}", e)))?;

    info!("Reconciled Deployment {}/{}", namespace, name);

    Ok(name)
}

/// Reconcile the Service for proxy access
pub async fn reconcile_service(
    remapper: &KafkaPartitionRemapper,
    client: &Client,
    namespace: &str,
) -> Result<String> {
    let name = remapper.name_any();

    // Build Service
    let service = service_builder::build_service(remapper);

    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    let patch_params = PatchParams::apply("kafka-partition-remapper-operator");

    services
        .patch(&name, &patch_params, &Patch::Apply(&service))
        .await
        .map_err(|e| Error::KubeError(format!("Failed to create/update Service: {}", e)))?;

    info!("Reconciled Service {}/{}", namespace, name);

    Ok(name)
}

/// Update the status of a KafkaPartitionRemapper
pub async fn update_status(
    remapper: &KafkaPartitionRemapper,
    client: &Client,
    namespace: &str,
    config_map_name: &str,
    deployment_name: &str,
    service_name: &str,
) -> Result<()> {
    let name = remapper.name_any();
    let spec = &remapper.spec;

    // Get deployment status
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let deployment = deployments.get(deployment_name).await.ok();

    let (ready_replicas, replicas) = deployment
        .as_ref()
        .and_then(|d| d.status.as_ref())
        .map(|s| (s.ready_replicas.unwrap_or(0), s.replicas.unwrap_or(0)))
        .unwrap_or((0, 0));

    // Get service endpoint
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    let service = services.get(service_name).await.ok();
    let service_endpoint = service
        .as_ref()
        .and_then(|s| service_builder::get_service_endpoint(s, spec));

    // Determine phase
    let phase = if spec.suspend {
        "Suspended"
    } else if ready_replicas == spec.replicas {
        "Running"
    } else if ready_replicas > 0 {
        "Degraded"
    } else {
        "Pending"
    };

    // Build conditions
    let mut conditions = Vec::new();
    let now = Utc::now();

    conditions.push(Condition {
        type_: "ConfigValid".to_string(),
        status: "True".to_string(),
        last_transition_time: now,
        reason: Some("ConfigurationValid".to_string()),
        message: Some("Configuration is valid".to_string()),
    });

    conditions.push(Condition {
        type_: "DeploymentAvailable".to_string(),
        status: if ready_replicas > 0 { "True" } else { "False" }.to_string(),
        last_transition_time: now,
        reason: Some(
            if ready_replicas > 0 {
                "ReplicasAvailable"
            } else {
                "NoReplicasAvailable"
            }
            .to_string(),
        ),
        message: Some(format!(
            "{}/{} replicas ready",
            ready_replicas, spec.replicas
        )),
    });

    conditions.push(Condition {
        type_: "Ready".to_string(),
        status: if phase == "Running" { "True" } else { "False" }.to_string(),
        last_transition_time: now,
        reason: Some(phase.to_string()),
        message: Some(format!("Proxy is {}", phase.to_lowercase())),
    });

    // Calculate compression ratio
    let compression_ratio = spec.mapping.virtual_partitions / spec.mapping.physical_partitions;

    // Build status
    let status = KafkaPartitionRemapperStatus {
        phase: Some(phase.to_string()),
        message: Some(format!(
            "{}/{} replicas ready, compression ratio {}:1",
            ready_replicas, spec.replicas, compression_ratio
        )),
        service_endpoint,
        metrics_endpoint: Some(format!(
            "http://{}.{}.svc.cluster.local:{}/metrics",
            service_name, namespace, spec.metrics.port
        )),
        ready_replicas: Some(ready_replicas),
        replicas: Some(replicas),
        config_map_name: Some(config_map_name.to_string()),
        deployment_name: Some(deployment_name.to_string()),
        service_name: Some(service_name.to_string()),
        compression_ratio: Some(compression_ratio),
        observed_generation: remapper.metadata.generation,
        last_update_time: Some(now),
        conditions,
    };

    // Patch status
    let remappers: Api<KafkaPartitionRemapper> = Api::namespaced(client.clone(), namespace);
    let patch = serde_json::json!({
        "status": status
    });

    remappers
        .patch_status(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map_err(|e| Error::KubeError(format!("Failed to update status: {}", e)))?;

    info!(
        "Updated status for {}/{}: phase={}, ready={}/{}",
        namespace, name, phase, ready_replicas, spec.replicas
    );

    Ok(())
}

/// Calculate a hash of the configuration for rolling updates
fn calculate_config_hash(remapper: &KafkaPartitionRemapper) -> String {
    let mut hasher = Sha256::new();
    let spec_json = serde_json::to_string(&remapper.spec).unwrap_or_default();
    hasher.update(spec_json.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn build_owner_reference(
    remapper: &KafkaPartitionRemapper,
) -> k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference {
    k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference {
        api_version: "kafka.oso.sh/v1alpha1".to_string(),
        kind: "KafkaPartitionRemapper".to_string(),
        name: remapper.name_any(),
        uid: remapper.metadata.uid.clone().unwrap_or_default(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    }
}
