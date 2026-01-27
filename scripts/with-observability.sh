#!/usr/bin/env bash
# Start the observability stack (Prometheus, Grafana, Jaeger) for local dev.
# Ensure the flowraft network exists (create if not): e.g. after docker compose up from the main docker-compose.yml.
# Usage: ./scripts/with-observability.sh

set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

docker network create flowraft 2>/dev/null || true
docker compose -f opentelemetry/docker-compose.yml up -d
echo "Prometheus :9093, Grafana :3000 (admin/admin), Jaeger :16686"
