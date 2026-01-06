//! Integration tests for reconciler validation logic
//!
//! These tests verify that the validation functions for KafkaPartitionRemapper
//! correctly accept valid specs and reject invalid ones.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kafka_partition_remapper_operator::crd::{
    KafkaClusterSpec, KafkaPartitionRemapper, KafkaPartitionRemapperSpec, ListenSpec,
    LoggingSpec, MappingSpec, MetricsSpec, ServiceSpec,
};
use kafka_partition_remapper_operator::reconcilers::remapper;

// ============================================================================
// Test Helpers
// ============================================================================

fn valid_kafka_cluster() -> KafkaClusterSpec {
    KafkaClusterSpec {
        bootstrap_servers: vec!["kafka:9092".to_string()],
        security_protocol: "PLAINTEXT".to_string(),
        tls_secret: None,
        sasl_secret: None,
        connection_timeout_ms: 10000,
        request_timeout_ms: 30000,
        metadata_refresh_interval_secs: 30,
    }
}

fn valid_listen_spec() -> ListenSpec {
    ListenSpec {
        port: 9092,
        max_connections: 1000,
        advertised_address: None,
        security: None,
    }
}

fn valid_mapping_spec() -> MappingSpec {
    MappingSpec {
        virtual_partitions: 1000,
        physical_partitions: 100,
        offset_range: 1 << 40, // 2^40
        topics: vec![],
    }
}

fn default_metadata(name: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.to_string()),
        namespace: Some("default".to_string()),
        ..Default::default()
    }
}

fn valid_remapper_spec() -> KafkaPartitionRemapperSpec {
    KafkaPartitionRemapperSpec {
        replicas: 2,
        listen: valid_listen_spec(),
        kafka: valid_kafka_cluster(),
        mapping: valid_mapping_spec(),
        metrics: MetricsSpec {
            enabled: true,
            port: 9090,
        },
        logging: LoggingSpec {
            level: "info".to_string(),
            json: false,
        },
        service: ServiceSpec {
            type_: "ClusterIP".to_string(),
            annotations: Default::default(),
            load_balancer_ip: None,
            external_traffic_policy: None,
        },
        pod_template: None,
        suspend: false,
    }
}

fn create_remapper(spec: KafkaPartitionRemapperSpec) -> KafkaPartitionRemapper {
    KafkaPartitionRemapper {
        metadata: default_metadata("test-remapper"),
        spec,
        status: None,
    }
}

// ============================================================================
// Basic Validation Tests
// ============================================================================

#[test]
fn remapper_valid_spec_passes_validation() {
    let remapper = create_remapper(valid_remapper_spec());
    let result = remapper::validate(&remapper);
    if let Err(e) = &result {
        panic!("Validation failed unexpectedly: {:?}", e);
    }
    assert!(result.is_ok());
}

#[test]
fn remapper_empty_bootstrap_servers_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.kafka.bootstrap_servers = vec![];

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("bootstrap"));
}

// ============================================================================
// Partition Mapping Validation Tests
// ============================================================================

#[test]
fn remapper_zero_physical_partitions_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.mapping.physical_partitions = 0;

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("physical"));
}

#[test]
fn remapper_virtual_less_than_physical_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.mapping.physical_partitions = 100;
    spec.mapping.virtual_partitions = 50; // Less than physical

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("virtual"));
}

#[test]
fn remapper_virtual_not_divisible_by_physical_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.mapping.physical_partitions = 100;
    spec.mapping.virtual_partitions = 1001; // Not evenly divisible

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("divisible"));
}

#[test]
fn remapper_valid_partition_ratios_pass_validation() {
    let valid_ratios = vec![
        (100, 100),   // 1:1
        (100, 200),   // 1:2
        (100, 1000),  // 1:10
        (50, 500),    // 1:10
        (10, 100),    // 1:10
    ];

    for (physical, virtual_p) in valid_ratios {
        let mut spec = valid_remapper_spec();
        spec.mapping.physical_partitions = physical;
        spec.mapping.virtual_partitions = virtual_p;

        let remapper = create_remapper(spec);
        assert!(
            remapper::validate(&remapper).is_ok(),
            "Ratio {}:{} should be valid",
            physical,
            virtual_p
        );
    }
}

#[test]
fn remapper_offset_range_too_small_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.mapping.offset_range = 1000; // Less than 2^20

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("offset"));
}

#[test]
fn remapper_valid_offset_ranges_pass_validation() {
    let valid_ranges = vec![
        1 << 20, // Minimum: 2^20
        1 << 30, // 2^30
        1 << 40, // 2^40
    ];

    for range in valid_ranges {
        let mut spec = valid_remapper_spec();
        spec.mapping.offset_range = range;

        let remapper = create_remapper(spec);
        assert!(
            remapper::validate(&remapper).is_ok(),
            "Offset range {} should be valid",
            range
        );
    }
}

// ============================================================================
// Replica Validation Tests
// ============================================================================

#[test]
fn remapper_negative_replicas_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.replicas = -1;

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("replica"));
}

#[test]
fn remapper_zero_replicas_passes_validation() {
    let mut spec = valid_remapper_spec();
    spec.replicas = 0;

    let remapper = create_remapper(spec);
    assert!(remapper::validate(&remapper).is_ok());
}

#[test]
fn remapper_valid_replicas_pass_validation() {
    for replicas in [1, 2, 3, 5, 10] {
        let mut spec = valid_remapper_spec();
        spec.replicas = replicas;

        let remapper = create_remapper(spec);
        assert!(
            remapper::validate(&remapper).is_ok(),
            "Replicas {} should be valid",
            replicas
        );
    }
}

// ============================================================================
// Security Protocol Validation Tests
// ============================================================================

#[test]
fn remapper_invalid_security_protocol_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.kafka.security_protocol = "INVALID".to_string();

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("security"));
}

#[test]
fn remapper_valid_security_protocols_pass_validation() {
    let valid_protocols = vec!["PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL"];

    for protocol in valid_protocols {
        let mut spec = valid_remapper_spec();
        spec.kafka.security_protocol = protocol.to_string();

        // Add required secrets for protocols that need them
        if protocol.contains("SSL") {
            spec.kafka.tls_secret = Some(kafka_partition_remapper_operator::crd::TlsSecretRef {
                name: "tls-secret".to_string(),
                ca_key: "ca.crt".to_string(),
                cert_key: None,
                key_key: None,
                insecure_skip_verify: false,
            });
        }
        if protocol.contains("SASL") {
            spec.kafka.sasl_secret = Some(kafka_partition_remapper_operator::crd::SaslSecretRef {
                name: "sasl-secret".to_string(),
                mechanism: "PLAIN".to_string(),
                username_key: "username".to_string(),
                password_key: "password".to_string(),
            });
        }

        let remapper = create_remapper(spec);
        assert!(
            remapper::validate(&remapper).is_ok(),
            "Protocol '{}' should be valid",
            protocol
        );
    }
}

#[test]
fn remapper_ssl_without_tls_secret_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.kafka.security_protocol = "SSL".to_string();
    spec.kafka.tls_secret = None;

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("tls"));
}

#[test]
fn remapper_sasl_ssl_without_sasl_secret_fails_validation() {
    let mut spec = valid_remapper_spec();
    spec.kafka.security_protocol = "SASL_SSL".to_string();
    spec.kafka.tls_secret = Some(kafka_partition_remapper_operator::crd::TlsSecretRef {
        name: "tls-secret".to_string(),
        ca_key: "ca.crt".to_string(),
        cert_key: None,
        key_key: None,
        insecure_skip_verify: false,
    });
    spec.kafka.sasl_secret = None;

    let remapper = create_remapper(spec);
    let result = remapper::validate(&remapper);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("sasl"));
}

// ============================================================================
// Suspend Mode Tests
// ============================================================================

#[test]
fn remapper_suspended_mode_passes_validation() {
    let mut spec = valid_remapper_spec();
    spec.suspend = true;

    let remapper = create_remapper(spec);
    assert!(remapper::validate(&remapper).is_ok());
}
