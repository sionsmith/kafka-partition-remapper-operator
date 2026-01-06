//! CRD YAML Generator
//!
//! This binary generates Kubernetes CRD manifests for all custom resources
//! defined by the kafka-partition-remapper-operator.
//!
//! Usage: cargo run --bin crdgen > deploy/crds/all.yaml

use kafka_partition_remapper_operator::crd::generate_crds;

fn main() {
    for crd in generate_crds() {
        println!("---");
        print!("{}", crd);
    }
}
