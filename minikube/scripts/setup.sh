#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MINIKUBE_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$MINIKUBE_DIR")"

echo "======================================================"
echo "Kafka Partition Remapper Operator - Minikube Test Setup"
echo "======================================================"

# Check if minikube is running
if ! minikube status &>/dev/null; then
    echo "Starting minikube..."
    minikube start --memory 8192 --cpus 4 --disk-size 20g --driver=docker
else
    echo "Minikube is already running"
fi

# Enable addons
echo "Enabling minikube addons..."
minikube addons enable storage-provisioner
minikube addons enable default-storageclass

echo ""
echo "Step 1: Adding Helm repositories..."
echo "------------------------------------"
helm repo add confluentinc https://packages.confluent.io/helm || true
helm repo update

echo ""
echo "Step 2: Installing Confluent for Kubernetes via Helm..."
echo "--------------------------------------------------------"
kubectl create namespace confluent --dry-run=client -o yaml | kubectl apply -f -
helm upgrade --install confluent-operator confluentinc/confluent-for-kubernetes \
    --namespace confluent \
    --wait \
    --timeout 5m

echo ""
echo "Step 3: Installing Kafka Partition Remapper Operator CRDs..."
echo "-------------------------------------------------------------"
kubectl apply -f "$REPO_ROOT/deploy/crds/all.yaml"

echo ""
echo "Step 4: Building and loading operator image into minikube..."
echo "-------------------------------------------------------------"
# Build the image using minikube's Docker daemon
eval $(minikube docker-env)
docker build -t ghcr.io/osodevops/kafka-partition-remapper-operator:latest -f "$REPO_ROOT/Dockerfile" "$REPO_ROOT"

echo ""
echo "Step 5: Installing Kafka Partition Remapper Operator via Helm..."
echo "-----------------------------------------------------------------"
helm upgrade --install kafka-partition-remapper-operator "$REPO_ROOT/deploy/helm/kafka-partition-remapper-operator" \
    --namespace confluent \
    --set image.pullPolicy=Never \
    --set image.tag=latest \
    --wait \
    --timeout 2m

echo ""
echo "Step 6: Deploying Confluent Platform (ZK + Kafka)..."
echo "-----------------------------------------------------"
kubectl apply -k "$MINIKUBE_DIR/overlays/test"

echo ""
echo "Waiting for Zookeeper to be ready..."
kubectl wait --for=condition=Ready pod/zookeeper-0 -n confluent --timeout=300s || true

echo ""
echo "Waiting for Kafka to be ready..."
kubectl wait --for=condition=Ready pod/kafka-0 -n confluent --timeout=300s || true

echo ""
echo "Waiting for Partition Remapper proxy pods..."
sleep 10
kubectl wait --for=condition=Ready pod -l app.kubernetes.io/name=kafka-partition-remapper -n confluent --timeout=120s || true

echo ""
echo "======================================================"
echo "Setup complete!"
echo "======================================================"
echo ""
echo "To check the status of all components:"
echo "  kubectl get pods -n confluent"
echo ""
echo "To check the KafkaPartitionRemapper status:"
echo "  kubectl get kafkapartitionremapper -n confluent"
echo "  kubectl describe kafkapartitionremapper test-remapper -n confluent"
echo ""
echo "To view producer logs (messages through proxy):"
echo "  kubectl logs -f deployment/kafka-producer -n confluent"
echo ""
echo "To view consumer logs (reading through proxy):"
echo "  kubectl logs -f deployment/kafka-consumer -n confluent"
echo ""
echo "To view operator logs:"
echo "  kubectl logs -f deployment/kafka-partition-remapper-operator -n confluent"
echo ""
echo "To view proxy logs:"
echo "  kubectl logs -f deployment/test-remapper -n confluent"
echo ""
echo "To verify the setup:"
echo "  ./scripts/verify.sh"
