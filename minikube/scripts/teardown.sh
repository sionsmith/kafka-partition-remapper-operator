#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MINIKUBE_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$MINIKUBE_DIR")"

echo "======================================================"
echo "Tearing down Kafka Partition Remapper test environment"
echo "======================================================"

echo ""
echo "Deleting test resources..."
kubectl delete -k "$MINIKUBE_DIR/overlays/test" --ignore-not-found=true

echo ""
echo "Deleting Kafka Partition Remapper Operator..."
helm uninstall kafka-partition-remapper-operator -n confluent --ignore-not-found || true
kubectl delete -f "$REPO_ROOT/deploy/crds/all.yaml" --ignore-not-found=true

echo ""
echo "Deleting Confluent Operator..."
helm uninstall confluent-operator -n confluent --ignore-not-found || true

echo ""
echo "Deleting namespace..."
kubectl delete namespace confluent --ignore-not-found=true

echo ""
echo "======================================================"
echo "Teardown complete!"
echo "======================================================"
echo ""
echo "To completely reset, run:"
echo "  minikube delete && minikube start --memory 8192 --cpus 4"
