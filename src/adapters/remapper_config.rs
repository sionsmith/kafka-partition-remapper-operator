//! CRD spec to proxy YAML configuration transformation

use crate::crd::KafkaPartitionRemapperSpec;
use crate::Result;

/// Build the proxy YAML configuration from CRD spec
pub fn build_proxy_config(
    spec: &KafkaPartitionRemapperSpec,
    advertised_address: &str,
) -> Result<String> {
    // Build the YAML configuration that the proxy expects
    let mut config = serde_yaml::Mapping::new();

    // Listen configuration
    let mut listen = serde_yaml::Mapping::new();
    listen.insert(
        serde_yaml::Value::String("address".to_string()),
        serde_yaml::Value::String(format!("0.0.0.0:{}", spec.listen.port)),
    );
    listen.insert(
        serde_yaml::Value::String("advertised_address".to_string()),
        serde_yaml::Value::String(advertised_address.to_string()),
    );
    listen.insert(
        serde_yaml::Value::String("max_connections".to_string()),
        serde_yaml::Value::Number(spec.listen.max_connections.into()),
    );
    config.insert(
        serde_yaml::Value::String("listen".to_string()),
        serde_yaml::Value::Mapping(listen),
    );

    // Kafka configuration
    let mut kafka = serde_yaml::Mapping::new();
    let bootstrap_servers: Vec<serde_yaml::Value> = spec
        .kafka
        .bootstrap_servers
        .iter()
        .map(|s| serde_yaml::Value::String(s.clone()))
        .collect();
    kafka.insert(
        serde_yaml::Value::String("bootstrap_servers".to_string()),
        serde_yaml::Value::Sequence(bootstrap_servers),
    );
    kafka.insert(
        serde_yaml::Value::String("connection_timeout_ms".to_string()),
        serde_yaml::Value::Number(spec.kafka.connection_timeout_ms.into()),
    );
    kafka.insert(
        serde_yaml::Value::String("request_timeout_ms".to_string()),
        serde_yaml::Value::Number(spec.kafka.request_timeout_ms.into()),
    );
    kafka.insert(
        serde_yaml::Value::String("metadata_refresh_interval_secs".to_string()),
        serde_yaml::Value::Number(spec.kafka.metadata_refresh_interval_secs.into()),
    );
    kafka.insert(
        serde_yaml::Value::String("security_protocol".to_string()),
        serde_yaml::Value::String(spec.kafka.security_protocol.clone()),
    );
    config.insert(
        serde_yaml::Value::String("kafka".to_string()),
        serde_yaml::Value::Mapping(kafka),
    );

    // Mapping configuration
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(
        serde_yaml::Value::String("virtual_partitions".to_string()),
        serde_yaml::Value::Number(spec.mapping.virtual_partitions.into()),
    );
    mapping.insert(
        serde_yaml::Value::String("physical_partitions".to_string()),
        serde_yaml::Value::Number(spec.mapping.physical_partitions.into()),
    );
    mapping.insert(
        serde_yaml::Value::String("offset_range".to_string()),
        serde_yaml::Value::Number(spec.mapping.offset_range.into()),
    );

    // Per-topic overrides
    if !spec.mapping.topics.is_empty() {
        let mut topics = serde_yaml::Mapping::new();
        for topic_override in &spec.mapping.topics {
            let mut topic_config = serde_yaml::Mapping::new();
            if let Some(vp) = topic_override.virtual_partitions {
                topic_config.insert(
                    serde_yaml::Value::String("virtual_partitions".to_string()),
                    serde_yaml::Value::Number(vp.into()),
                );
            }
            if let Some(pp) = topic_override.physical_partitions {
                topic_config.insert(
                    serde_yaml::Value::String("physical_partitions".to_string()),
                    serde_yaml::Value::Number(pp.into()),
                );
            }
            if let Some(or) = topic_override.offset_range {
                topic_config.insert(
                    serde_yaml::Value::String("offset_range".to_string()),
                    serde_yaml::Value::Number(or.into()),
                );
            }
            topics.insert(
                serde_yaml::Value::String(topic_override.topic.clone()),
                serde_yaml::Value::Mapping(topic_config),
            );
        }
        mapping.insert(
            serde_yaml::Value::String("topics".to_string()),
            serde_yaml::Value::Mapping(topics),
        );
    }
    config.insert(
        serde_yaml::Value::String("mapping".to_string()),
        serde_yaml::Value::Mapping(mapping),
    );

    // Metrics configuration
    let mut metrics = serde_yaml::Mapping::new();
    metrics.insert(
        serde_yaml::Value::String("enabled".to_string()),
        serde_yaml::Value::Bool(spec.metrics.enabled),
    );
    metrics.insert(
        serde_yaml::Value::String("address".to_string()),
        serde_yaml::Value::String(format!("0.0.0.0:{}", spec.metrics.port)),
    );
    config.insert(
        serde_yaml::Value::String("metrics".to_string()),
        serde_yaml::Value::Mapping(metrics),
    );

    // Logging configuration
    let mut logging = serde_yaml::Mapping::new();
    logging.insert(
        serde_yaml::Value::String("level".to_string()),
        serde_yaml::Value::String(spec.logging.level.clone()),
    );
    logging.insert(
        serde_yaml::Value::String("json".to_string()),
        serde_yaml::Value::Bool(spec.logging.json),
    );
    config.insert(
        serde_yaml::Value::String("logging".to_string()),
        serde_yaml::Value::Mapping(logging),
    );

    serde_yaml::to_string(&serde_yaml::Value::Mapping(config))
        .map_err(|e| crate::Error::ConfigError(format!("Failed to serialize config: {}", e)))
}
