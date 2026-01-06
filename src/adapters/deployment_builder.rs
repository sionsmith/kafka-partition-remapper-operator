//! Kubernetes Deployment builder for proxy pods

use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, Probe, TCPSocketAction,
    Volume, VolumeMount, ConfigMapVolumeSource, LocalObjectReference,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use std::collections::BTreeMap;

use crate::crd::{KafkaPartitionRemapper, KafkaPartitionRemapperSpec};

const DEFAULT_IMAGE: &str = "ghcr.io/osodevops/kafka-partition-remapper";
const DEFAULT_TAG: &str = "latest";

/// Build a Deployment for the proxy
pub fn build_deployment(
    remapper: &KafkaPartitionRemapper,
    config_map_name: &str,
    config_hash: &str,
) -> Deployment {
    let name = remapper.metadata.name.clone().unwrap_or_default();
    let namespace = remapper.metadata.namespace.clone().unwrap_or_default();
    let spec = &remapper.spec;

    let labels = build_labels(&name);
    let mut pod_annotations = BTreeMap::new();
    pod_annotations.insert("checksum/config".to_string(), config_hash.to_string());

    // Merge user-provided pod template annotations
    if let Some(ref pt) = spec.pod_template {
        for (k, v) in &pt.annotations {
            pod_annotations.insert(k.clone(), v.clone());
        }
    }

    let replicas = if spec.suspend { 0 } else { spec.replicas };

    Deployment {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace),
            labels: Some(labels.clone()),
            owner_references: Some(vec![build_owner_reference(remapper)]),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(replicas),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels.clone()),
                    annotations: Some(pod_annotations),
                    ..Default::default()
                }),
                spec: Some(build_pod_spec(spec, config_map_name)),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn build_pod_spec(spec: &KafkaPartitionRemapperSpec, config_map_name: &str) -> PodSpec {
    let image = spec
        .pod_template
        .as_ref()
        .and_then(|pt| pt.image.clone())
        .unwrap_or_else(|| DEFAULT_IMAGE.to_string());

    let tag = spec
        .pod_template
        .as_ref()
        .and_then(|pt| pt.image_tag.clone())
        .unwrap_or_else(|| DEFAULT_TAG.to_string());

    let image_pull_policy = spec
        .pod_template
        .as_ref()
        .and_then(|pt| pt.image_pull_policy.clone())
        .unwrap_or_else(|| "IfNotPresent".to_string());

    let mut container = Container {
        name: "proxy".to_string(),
        image: Some(format!("{}:{}", image, tag)),
        image_pull_policy: Some(image_pull_policy),
        args: Some(vec![
            "--config".to_string(),
            "/etc/kafka-proxy/config.yaml".to_string(),
        ]),
        ports: Some(vec![
            ContainerPort {
                name: Some("kafka".to_string()),
                container_port: spec.listen.port,
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
            ContainerPort {
                name: Some("metrics".to_string()),
                container_port: spec.metrics.port,
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
        ]),
        volume_mounts: Some(vec![VolumeMount {
            name: "config".to_string(),
            mount_path: "/etc/kafka-proxy".to_string(),
            read_only: Some(true),
            ..Default::default()
        }]),
        liveness_probe: Some(Probe {
            tcp_socket: Some(TCPSocketAction {
                port: IntOrString::String("kafka".to_string()),
                ..Default::default()
            }),
            initial_delay_seconds: Some(10),
            period_seconds: Some(10),
            timeout_seconds: Some(5),
            failure_threshold: Some(3),
            ..Default::default()
        }),
        readiness_probe: Some(Probe {
            tcp_socket: Some(TCPSocketAction {
                port: IntOrString::String("kafka".to_string()),
                ..Default::default()
            }),
            initial_delay_seconds: Some(5),
            period_seconds: Some(5),
            timeout_seconds: Some(3),
            failure_threshold: Some(3),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Add resource requirements if specified
    if let Some(ref pt) = spec.pod_template {
        if let Some(ref resources) = pt.resources {
            container.resources = Some(k8s_openapi::api::core::v1::ResourceRequirements {
                limits: if resources.limits.is_empty() {
                    None
                } else {
                    Some(
                        resources
                            .limits
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(
                                        v.clone(),
                                    ),
                                )
                            })
                            .collect(),
                    )
                },
                requests: if resources.requests.is_empty() {
                    None
                } else {
                    Some(
                        resources
                            .requests
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(
                                        v.clone(),
                                    ),
                                )
                            })
                            .collect(),
                    )
                },
                ..Default::default()
            });
        }
    }

    // Add environment variables for SASL credentials if configured
    let mut env_vars = Vec::new();
    if let Some(ref sasl) = spec.kafka.sasl_secret {
        env_vars.push(EnvVar {
            name: "KAFKA_USERNAME".to_string(),
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                secret_key_ref: Some(k8s_openapi::api::core::v1::SecretKeySelector {
                    name: sasl.name.clone(),
                    key: sasl.username_key.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });
        env_vars.push(EnvVar {
            name: "KAFKA_PASSWORD".to_string(),
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                secret_key_ref: Some(k8s_openapi::api::core::v1::SecretKeySelector {
                    name: sasl.name.clone(),
                    key: sasl.password_key.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    if !env_vars.is_empty() {
        container.env = Some(env_vars);
    }

    let mut volumes = vec![Volume {
        name: "config".to_string(),
        config_map: Some(ConfigMapVolumeSource {
            name: config_map_name.to_string(),
            ..Default::default()
        }),
        ..Default::default()
    }];

    // Add TLS volume mounts if configured
    let mut volume_mounts = container.volume_mounts.take().unwrap_or_default();
    if let Some(ref tls) = spec.kafka.tls_secret {
        volumes.push(Volume {
            name: "kafka-tls".to_string(),
            secret: Some(k8s_openapi::api::core::v1::SecretVolumeSource {
                secret_name: Some(tls.name.clone()),
                ..Default::default()
            }),
            ..Default::default()
        });
        volume_mounts.push(VolumeMount {
            name: "kafka-tls".to_string(),
            mount_path: "/etc/kafka-proxy/tls/kafka".to_string(),
            read_only: Some(true),
            ..Default::default()
        });
    }
    container.volume_mounts = Some(volume_mounts);

    let mut pod_spec = PodSpec {
        containers: vec![container],
        volumes: Some(volumes),
        ..Default::default()
    };

    // Apply pod template settings
    if let Some(ref pt) = spec.pod_template {
        if !pt.node_selector.is_empty() {
            pod_spec.node_selector = Some(pt.node_selector.clone());
        }

        if !pt.tolerations.is_empty() {
            pod_spec.tolerations = Some(
                pt.tolerations
                    .iter()
                    .map(|t| k8s_openapi::api::core::v1::Toleration {
                        key: t.key.clone(),
                        operator: t.operator.clone(),
                        value: t.value.clone(),
                        effect: t.effect.clone(),
                        toleration_seconds: t.toleration_seconds,
                    })
                    .collect(),
            );
        }

        if let Some(ref sa) = pt.service_account_name {
            pod_spec.service_account_name = Some(sa.clone());
        }

        if !pt.image_pull_secrets.is_empty() {
            pod_spec.image_pull_secrets = Some(
                pt.image_pull_secrets
                    .iter()
                    .map(|s| LocalObjectReference {
                        name: s.clone(),
                    })
                    .collect(),
            );
        }
    }

    pod_spec
}

fn build_labels(name: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert(
        "app.kubernetes.io/name".to_string(),
        "kafka-partition-remapper".to_string(),
    );
    labels.insert("app.kubernetes.io/instance".to_string(), name.to_string());
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "kafka-partition-remapper-operator".to_string(),
    );
    labels
}

fn build_owner_reference(remapper: &KafkaPartitionRemapper) -> OwnerReference {
    OwnerReference {
        api_version: "kafka.oso.sh/v1alpha1".to_string(),
        kind: "KafkaPartitionRemapper".to_string(),
        name: remapper.metadata.name.clone().unwrap_or_default(),
        uid: remapper.metadata.uid.clone().unwrap_or_default(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    }
}
