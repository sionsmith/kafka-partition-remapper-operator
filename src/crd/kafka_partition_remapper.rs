//! KafkaPartitionRemapper Custom Resource Definition

use chrono::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// KafkaPartitionRemapper resource specification
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "kafka.oso.sh",
    version = "v1alpha1",
    kind = "KafkaPartitionRemapper",
    plural = "kafkapartitionremappers",
    singular = "kafkapartitionremapper",
    shortname = "kpr",
    namespaced,
    status = "KafkaPartitionRemapperStatus",
    printcolumn = r#"{"name": "Phase", "type": "string", "jsonPath": ".status.phase"}"#,
    printcolumn = r#"{"name": "Ready", "type": "string", "jsonPath": ".status.readyReplicas"}"#,
    printcolumn = r#"{"name": "Replicas", "type": "integer", "jsonPath": ".spec.replicas"}"#,
    printcolumn = r#"{"name": "Endpoint", "type": "string", "jsonPath": ".status.serviceEndpoint"}"#,
    printcolumn = r#"{"name": "Ratio", "type": "string", "jsonPath": ".status.compressionRatio"}"#,
    printcolumn = r#"{"name": "Age", "type": "date", "jsonPath": ".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct KafkaPartitionRemapperSpec {
    /// Number of proxy replicas for high availability
    #[serde(default = "default_replicas")]
    pub replicas: i32,

    /// TCP listener configuration for client connections
    pub listen: ListenSpec,

    /// Kafka cluster connection configuration
    pub kafka: KafkaClusterSpec,

    /// Partition remapping configuration
    pub mapping: MappingSpec,

    /// Prometheus metrics configuration
    #[serde(default)]
    pub metrics: MetricsSpec,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingSpec,

    /// Kubernetes Service configuration
    #[serde(default)]
    pub service: ServiceSpec,

    /// Pod template customizations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_template: Option<PodTemplateSpec>,

    /// Suspend proxy (scale to 0)
    #[serde(default)]
    pub suspend: bool,
}

fn default_replicas() -> i32 {
    1
}

/// TCP listener configuration for client connections
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListenSpec {
    /// Port to listen on (container port)
    #[serde(default = "default_listen_port")]
    pub port: i32,

    /// Advertised address for client reconnections
    /// If not set, uses the Service endpoint automatically
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advertised_address: Option<String>,

    /// Maximum concurrent client connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Client-facing security configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<ClientSecuritySpec>,
}

fn default_listen_port() -> i32 {
    9092
}

fn default_max_connections() -> u32 {
    1000
}

/// Client-facing security configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientSecuritySpec {
    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL)
    #[serde(default = "default_security_protocol")]
    pub protocol: String,

    /// TLS configuration for client connections (server-side TLS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<ClientTlsSpec>,

    /// SASL authentication for clients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl: Option<ClientSaslSpec>,
}

fn default_security_protocol() -> String {
    "PLAINTEXT".to_string()
}

/// TLS configuration for client-facing connections
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientTlsSpec {
    /// Secret containing server certificate and key
    pub certificate_secret: TlsCertificateSecretRef,

    /// Secret containing CA certificate for client verification (mTLS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ca_secret: Option<SecretRef>,

    /// Require client certificates (mTLS mode)
    #[serde(default)]
    pub require_client_cert: bool,
}

/// Reference to TLS certificate secret
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TlsCertificateSecretRef {
    /// Secret name
    pub name: String,
    /// Key for certificate (default: tls.crt)
    #[serde(default = "default_tls_cert_key")]
    pub cert_key: String,
    /// Key for private key (default: tls.key)
    #[serde(default = "default_tls_key_key")]
    pub key_key: String,
}

fn default_tls_cert_key() -> String {
    "tls.crt".to_string()
}

fn default_tls_key_key() -> String {
    "tls.key".to_string()
}

/// Simple secret reference
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretRef {
    /// Secret name
    pub name: String,
    /// Key in secret (default: ca.crt)
    #[serde(default = "default_ca_key")]
    pub key: String,
}

fn default_ca_key() -> String {
    "ca.crt".to_string()
}

/// SASL authentication configuration for clients
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientSaslSpec {
    /// Enabled SASL mechanisms (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, OAUTHBEARER)
    #[serde(default = "default_sasl_mechanisms")]
    pub enabled_mechanisms: Vec<String>,

    /// Credentials secret reference (username/password pairs)
    pub credentials_secret: CredentialsSecretRef,
}

fn default_sasl_mechanisms() -> Vec<String> {
    vec!["PLAIN".to_string()]
}

/// Credentials secret reference
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsSecretRef {
    /// Secret name containing user credentials
    /// Format: each key is a username, value is the password
    pub name: String,
}

/// Kafka cluster connection configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KafkaClusterSpec {
    /// Bootstrap servers
    pub bootstrap_servers: Vec<String>,

    /// Connection timeout in milliseconds
    #[serde(default = "default_connection_timeout_ms")]
    pub connection_timeout_ms: u64,

    /// Request timeout in milliseconds
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,

    /// Metadata refresh interval in seconds (0 to disable)
    #[serde(default = "default_metadata_refresh_interval_secs")]
    pub metadata_refresh_interval_secs: u64,

    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL)
    #[serde(default = "default_security_protocol")]
    pub security_protocol: String,

    /// TLS configuration for broker connections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_secret: Option<TlsSecretRef>,

    /// SASL configuration for broker connections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_secret: Option<SaslSecretRef>,
}

fn default_connection_timeout_ms() -> u64 {
    10_000
}

fn default_request_timeout_ms() -> u64 {
    30_000
}

fn default_metadata_refresh_interval_secs() -> u64 {
    30
}

/// TLS secret reference for broker connections
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TlsSecretRef {
    /// Secret name
    pub name: String,
    /// CA certificate key in secret
    #[serde(default = "default_ca_key")]
    pub ca_key: String,
    /// Client certificate key in secret (for mTLS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_key: Option<String>,
    /// Client key key in secret (for mTLS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_key: Option<String>,
    /// Skip server verification (NOT recommended for production)
    #[serde(default)]
    pub insecure_skip_verify: bool,
}

/// SASL secret reference for broker connections
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaslSecretRef {
    /// Secret name
    pub name: String,
    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
    pub mechanism: String,
    /// Username key in secret
    #[serde(default = "default_username_key")]
    pub username_key: String,
    /// Password key in secret
    #[serde(default = "default_password_key")]
    pub password_key: String,
}

fn default_username_key() -> String {
    "username".to_string()
}

fn default_password_key() -> String {
    "password".to_string()
}

/// Partition remapping configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MappingSpec {
    /// Number of virtual partitions exposed to clients
    pub virtual_partitions: u32,

    /// Number of physical partitions in Kafka cluster
    pub physical_partitions: u32,

    /// Offset range per virtual partition group (default: 2^40)
    #[serde(default = "default_offset_range")]
    pub offset_range: u64,

    /// Per-topic mapping overrides
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<TopicMappingOverride>,
}

fn default_offset_range() -> u64 {
    1 << 40 // 2^40 = 1,099,511,627,776
}

/// Per-topic mapping override
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopicMappingOverride {
    /// Topic name or regex pattern
    pub topic: String,

    /// Virtual partitions for this topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_partitions: Option<u32>,

    /// Physical partitions for this topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_partitions: Option<u32>,

    /// Offset range for this topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_range: Option<u64>,
}

/// Metrics configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSpec {
    /// Enable metrics endpoint
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub port: i32,
}

impl Default for MetricsSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 9090,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_metrics_port() -> i32 {
    9090
}

/// Logging configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoggingSpec {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Output logs in JSON format
    #[serde(default)]
    pub json: bool,
}

impl Default for LoggingSpec {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json: false,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Kubernetes Service configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSpec {
    /// Service type (ClusterIP, LoadBalancer, NodePort)
    #[serde(default = "default_service_type")]
    #[serde(rename = "type")]
    pub type_: String,

    /// Service annotations (for cloud load balancer configuration)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,

    /// LoadBalancer IP (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_balancer_ip: Option<String>,

    /// External traffic policy (Cluster, Local)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_traffic_policy: Option<String>,
}

impl Default for ServiceSpec {
    fn default() -> Self {
        Self {
            type_: "ClusterIP".to_string(),
            annotations: BTreeMap::new(),
            load_balancer_ip: None,
            external_traffic_policy: None,
        }
    }
}

fn default_service_type() -> String {
    "ClusterIP".to_string()
}

/// Pod template customizations
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PodTemplateSpec {
    /// Pod annotations
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,

    /// Pod labels
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,

    /// Node selector
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub node_selector: BTreeMap<String, String>,

    /// Tolerations (JSON/YAML format matching k8s tolerations)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tolerations: Vec<TolerationSpec>,

    /// Affinity rules (JSON/YAML format matching k8s affinity)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<serde_json::Value>,

    /// Resource requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceRequirementsSpec>,

    /// Image override (defaults to ghcr.io/osodevops/kafka-partition-remapper)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Image tag override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_tag: Option<String>,

    /// Image pull policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<String>,

    /// Image pull secrets (list of secret names)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_pull_secrets: Vec<String>,

    /// Service account name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_name: Option<String>,

    /// Security context (JSON/YAML format matching k8s pod security context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_context: Option<serde_json::Value>,
}

/// Toleration specification
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TolerationSpec {
    /// Toleration key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Toleration operator (Exists, Equal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// Toleration value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Toleration effect (NoSchedule, PreferNoSchedule, NoExecute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,

    /// Toleration seconds (for NoExecute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toleration_seconds: Option<i64>,
}

/// Resource requirements specification
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRequirementsSpec {
    /// Resource limits (cpu, memory)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, String>,

    /// Resource requests (cpu, memory)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub requests: BTreeMap<String, String>,
}

/// KafkaPartitionRemapper status
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KafkaPartitionRemapperStatus {
    /// Current phase (Pending, Running, Degraded, Failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Service endpoint for client connections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_endpoint: Option<String>,

    /// Metrics endpoint URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics_endpoint: Option<String>,

    /// Current number of ready replicas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_replicas: Option<i32>,

    /// Total number of replicas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,

    /// ConfigMap name for proxy configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_map_name: Option<String>,

    /// Deployment name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_name: Option<String>,

    /// Service name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,

    /// Compression ratio (virtual/physical partitions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_ratio: Option<u32>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Last update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_time: Option<DateTime<Utc>>,

    /// Status conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

/// Status condition
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Condition type (Ready, ConfigValid, DeploymentAvailable, ServiceReady)
    #[serde(rename = "type")]
    pub type_: String,

    /// Status (True, False, Unknown)
    pub status: String,

    /// Last transition time
    pub last_transition_time: DateTime<Utc>,

    /// Reason for the condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
