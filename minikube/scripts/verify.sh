#!/bin/bash
set -e

echo "======================================================"
echo "Verifying Kafka Partition Remapper Setup"
echo "======================================================"

echo ""
echo "1. Checking pods..."
echo "-------------------"
kubectl get pods -n confluent

echo ""
echo "2. KafkaPartitionRemapper status..."
echo "------------------------------------"
kubectl get kafkapartitionremapper -n confluent -o wide

echo ""
echo "3. KafkaPartitionRemapper details..."
echo "-------------------------------------"
kubectl describe kafkapartitionremapper test-remapper -n confluent | grep -A 20 "Status:"

echo ""
echo "4. Proxy Service endpoint..."
echo "-----------------------------"
kubectl get svc test-remapper -n confluent -o wide

echo ""
echo "5. Checking proxy metrics..."
echo "-----------------------------"
PROXY_POD=$(kubectl get pods -n confluent -l app.kubernetes.io/instance=test-remapper -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -n "$PROXY_POD" ]; then
    echo "Fetching metrics from $PROXY_POD..."
    kubectl exec -n confluent "$PROXY_POD" -- curl -s http://localhost:9090/metrics 2>/dev/null | head -20 || echo "Metrics not available yet"
else
    echo "No proxy pod found yet"
fi

echo ""
echo "6. Recent producer logs..."
echo "---------------------------"
kubectl logs deployment/kafka-producer -n confluent --tail=10 2>/dev/null || echo "Producer not ready yet"

echo ""
echo "7. Recent consumer logs..."
echo "---------------------------"
kubectl logs deployment/kafka-consumer -n confluent --tail=10 2>/dev/null || echo "Consumer not ready yet"

echo ""
echo "======================================================"
echo "Verification complete!"
echo "======================================================"
