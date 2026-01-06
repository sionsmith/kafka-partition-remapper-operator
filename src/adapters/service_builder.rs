//! Kubernetes Service builder for proxy access

use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use std::collections::BTreeMap;

use crate::crd::{KafkaPartitionRemapper, KafkaPartitionRemapperSpec};

/// Build a Service for the proxy
pub fn build_service(remapper: &KafkaPartitionRemapper) -> Service {
    let name = remapper.metadata.name.clone().unwrap_or_default();
    let namespace = remapper.metadata.namespace.clone().unwrap_or_default();
    let spec = &remapper.spec;

    let labels = build_labels(&name);

    Service {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace),
            labels: Some(labels.clone()),
            annotations: if spec.service.annotations.is_empty() {
                None
            } else {
                Some(spec.service.annotations.clone())
            },
            owner_references: Some(vec![build_owner_reference(remapper)]),
            ..Default::default()
        },
        spec: Some(build_service_spec(spec, &labels)),
        ..Default::default()
    }
}

fn build_service_spec(
    spec: &KafkaPartitionRemapperSpec,
    selector: &BTreeMap<String, String>,
) -> ServiceSpec {
    let mut service_spec = ServiceSpec {
        type_: Some(spec.service.type_.clone()),
        selector: Some(selector.clone()),
        ports: Some(vec![
            ServicePort {
                name: Some("kafka".to_string()),
                port: spec.listen.port,
                target_port: Some(IntOrString::String("kafka".to_string())),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
            ServicePort {
                name: Some("metrics".to_string()),
                port: spec.metrics.port,
                target_port: Some(IntOrString::String("metrics".to_string())),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };

    if let Some(ref lb_ip) = spec.service.load_balancer_ip {
        service_spec.load_balancer_ip = Some(lb_ip.clone());
    }

    if let Some(ref policy) = spec.service.external_traffic_policy {
        service_spec.external_traffic_policy = Some(policy.clone());
    }

    service_spec
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

/// Get the service endpoint for advertised address
pub fn get_service_endpoint(
    service: &Service,
    spec: &KafkaPartitionRemapperSpec,
) -> Option<String> {
    let name = service.metadata.name.as_ref()?;
    let namespace = service.metadata.namespace.as_ref()?;
    let port = spec.listen.port;

    let service_spec = service.spec.as_ref()?;
    let service_type = service_spec.type_.as_deref().unwrap_or("ClusterIP");

    match service_type {
        "LoadBalancer" => {
            // Try to get external IP from status
            if let Some(ref status) = service.status {
                if let Some(ref lb) = status.load_balancer {
                    if let Some(ref ingress) = lb.ingress {
                        if let Some(first) = ingress.first() {
                            if let Some(ref ip) = first.ip {
                                return Some(format!("{}:{}", ip, port));
                            }
                            if let Some(ref hostname) = first.hostname {
                                return Some(format!("{}:{}", hostname, port));
                            }
                        }
                    }
                }
            }
            // Fallback to cluster DNS if LB not ready
            Some(format!("{}.{}.svc.cluster.local:{}", name, namespace, port))
        }
        "NodePort" => {
            // For NodePort, return cluster DNS as we don't know node IPs
            Some(format!("{}.{}.svc.cluster.local:{}", name, namespace, port))
        }
        _ => {
            // ClusterIP - use internal DNS
            Some(format!("{}.{}.svc.cluster.local:{}", name, namespace, port))
        }
    }
}
