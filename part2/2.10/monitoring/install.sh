#!/usr/bin/env bash
# Install the Chapter 3 monitoring stack with Helm.
# Run from the part2/2.10/ directory. Idempotent (helm upgrade --install).
# Versions are PINNED for reproducibility (k8s-monitoring chart is v4.x).
set -euo pipefail

cd "$(dirname "$0")"

PROM_CHART="prometheus-community/prometheus"
PROM_VERSION="29.27.0"
LOKI_CHART="grafana/loki"
LOKI_VERSION="7.3.0"
K8SMON_CHART="grafana/k8s-monitoring"
K8SMON_VERSION="4.4.0"
GRAFANA_CHART="grafana/grafana"
GRAFANA_VERSION="10.5.15"

echo "==> Adding helm repos + refreshing index"
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update

echo "==> Creating monitoring namespace"
kubectl create namespace monitoring --dry-run=client -o yaml | kubectl apply -f -

echo "==> Installing prometheus (metrics) ${PROM_VERSION}"
helm upgrade --install prom "$PROM_CHART" --version "$PROM_VERSION" \
  --namespace monitoring --create-namespace --values prom-values.yaml

echo "==> Installing loki (logs) ${LOKI_VERSION}"
helm upgrade --install loki "$LOKI_CHART" --version "$LOKI_VERSION" \
  --namespace monitoring --values loki-values.yaml

echo "==> Installing alloy / k8s-monitoring (pod logs) ${K8SMON_VERSION}"
helm upgrade --install k8smon "$K8SMON_CHART" --version "$K8SMON_VERSION" \
  --namespace monitoring --values k8smon-values.yaml

echo "==> Installing grafana (visualization) ${GRAFANA_VERSION}"
helm upgrade --install grafana "$GRAFANA_CHART" --version "$GRAFANA_VERSION" \
  --namespace monitoring --values grafana-values.yaml

echo
echo "==> Done. Check status:"
echo "  helm list --namespace monitoring"
echo "  kubectl get pods --namespace monitoring -w"
echo "  kubectl port-forward --namespace monitoring svc/grafana 3000:80   # then http://localhost:3000 admin/admin"