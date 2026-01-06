//! Custom Resource Definitions for the Kafka Partition Remapper Operator

mod kafka_partition_remapper;

pub use kafka_partition_remapper::*;

use kube::CustomResourceExt;

/// Generate CRD YAML manifests for all custom resources
pub fn generate_crds() -> Vec<String> {
    vec![serde_yaml::to_string(&KafkaPartitionRemapper::crd()).unwrap()]
}
