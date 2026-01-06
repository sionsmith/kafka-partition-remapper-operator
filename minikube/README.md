# Kafka Partition Remapper Operator - Minikube Testing Environment

This directory contains a complete local testing environment for validating the kafka-partition-remapper-operator using Minikube and Confluent for Kubernetes (CFK).

## Prerequisites

- [Minikube](https://minikube.sigs.k8s.io/docs/start/) installed
- [kubectl](https://kubernetes.io/docs/tasks/tools/) installed
- [Helm](https://helm.sh/docs/intro/install/) installed
- [kustomize](https://kubectl.docs.kubernetes.io/installation/kustomize/) (or kubectl with kustomize support)
- Docker or another container runtime
- Minimum 8GB RAM available for Minikube

## Quick Start

### 1. Start the Test Environment

```bash
cd minikube
./scripts/setup.sh
```

This script will:
1. Start Minikube (if not already running)
2. Install Confluent for Kubernetes CRDs
3. Deploy the Confluent Operator
4. Install Kafka Partition Remapper Operator CRDs
5. Build and load the operator image into Minikube
6. Deploy the Kafka Partition Remapper Operator via Helm
7. Deploy Zookeeper and Kafka (single node, no auth)
8. Create a test topic (`remapper-test-topic`) with 10 physical partitions
9. Deploy a `KafkaPartitionRemapper` exposing 100 virtual partitions
10. Start a producer that sends messages through the proxy
11. Start a consumer that reads messages through the proxy

### 2. Verify the Setup

```bash
# Check all pods are running
kubectl get pods -n confluent

# Check the KafkaPartitionRemapper status
kubectl get kafkapartitionremapper -n confluent
kubectl describe kafkapartitionremapper test-remapper -n confluent

# Run the verification script
./scripts/verify.sh
```

### 3. Monitor the Components

```bash
# View producer logs (sending through proxy)
kubectl logs -f deployment/kafka-producer -n confluent

# View consumer logs (reading through proxy)
kubectl logs -f deployment/kafka-consumer -n confluent

# View operator logs
kubectl logs -f deployment/kafka-partition-remapper-operator -n confluent

# View proxy logs
kubectl logs -f deployment/test-remapper -n confluent
```

### 4. Teardown

```bash
./scripts/teardown.sh
```

Or for a complete reset:

```bash
minikube delete && minikube start --memory 8192 --cpus 4
```

## Directory Structure

```
minikube/
├── base/
│   └── confluent-platform/     # Kafka + Zookeeper components
│       ├── kustomization.yaml
│       ├── zookeeper.yaml
│       ├── kafka.yaml
│       └── kafka-topic.yaml
├── overlays/
│   └── test/                   # Test environment overlay
│       ├── kustomization.yaml
│       ├── kafka-partition-remapper.yaml
│       ├── producer.yaml
│       └── consumer.yaml
├── scripts/
│   ├── setup.sh               # Full environment setup
│   ├── teardown.sh            # Environment cleanup
│   └── verify.sh              # Verify setup
└── README.md
```

## Test Scenario

The test environment creates:

1. **Kafka Cluster**: Single-node Kafka with PLAINTEXT (no authentication)
2. **Test Topic**: `remapper-test-topic` with 10 physical partitions
3. **Partition Remapper**: Proxy exposing 100 virtual partitions (10:1 ratio)
4. **Producer**: Sends messages through the proxy with varying keys
5. **Consumer**: Reads messages through the proxy

### How Partition Remapping Works

```
Client View (100 virtual partitions)     Physical Kafka (10 partitions)
┌─────────────────────────────────┐      ┌─────────────────────────────┐
│ Partition 0-9   → Physical 0    │      │                             │
│ Partition 10-19 → Physical 1    │      │  Topic: remapper-test-topic │
│ Partition 20-29 → Physical 2    │      │  Partitions: 0-9            │
│ ...                             │  →   │                             │
│ Partition 90-99 → Physical 9    │      │                             │
└─────────────────────────────────┘      └─────────────────────────────┘
         ↑                                          ↑
         │                                          │
    ┌────┴────┐                               ┌─────┴─────┐
    │  Proxy  │ ────────────────────────────→ │   Kafka   │
    └─────────┘                               └───────────┘
```

## Configuration

### Kafka Configuration

The test environment uses a simplified single-node Kafka setup with:
- No authentication (PLAINTEXT)
- Single replica (for local testing)
- Auto topic creation enabled

### KafkaPartitionRemapper Configuration

The `kafka-partition-remapper.yaml` creates a proxy with:
- **Replicas**: 2 (for HA testing)
- **Virtual Partitions**: 100
- **Physical Partitions**: 10
- **Compression Ratio**: 10:1
- **Service Type**: ClusterIP

## Troubleshooting

### Pods not starting

```bash
# Check pod events
kubectl describe pod <pod-name> -n confluent

# Check operator logs
kubectl logs deployment/confluent-operator -n confluent
kubectl logs deployment/kafka-partition-remapper-operator -n confluent
```

### Kafka not ready

Kafka requires Zookeeper to be healthy first:

```bash
kubectl wait --for=condition=Ready pod/zookeeper-0 -n confluent --timeout=300s
kubectl wait --for=condition=Ready pod/kafka-0 -n confluent --timeout=300s
```

### Proxy not routing

Check the proxy status and logs:

```bash
kubectl describe kafkapartitionremapper test-remapper -n confluent
kubectl logs deployment/test-remapper -n confluent -f
```

### Resource constraints

If pods are being OOMKilled or stuck pending, increase Minikube resources:

```bash
minikube delete
minikube start --memory 12288 --cpus 6
```

## Manual Testing

### Produce directly to Kafka (bypass proxy)

```bash
kubectl exec -it kafka-0 -n confluent -- kafka-console-producer \
  --bootstrap-server localhost:9071 \
  --topic remapper-test-topic
```

### Consume directly from Kafka (bypass proxy)

```bash
kubectl exec -it kafka-0 -n confluent -- kafka-console-consumer \
  --bootstrap-server localhost:9071 \
  --topic remapper-test-topic \
  --from-beginning
```

### Port-forward to proxy

```bash
kubectl port-forward svc/test-remapper -n confluent 9092:9092
# Then use kafka tools with localhost:9092
```

### Check proxy metrics

```bash
kubectl port-forward svc/test-remapper -n confluent 9090:9090
curl http://localhost:9090/metrics
```
