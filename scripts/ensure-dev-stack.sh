#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p "${HOME}/.nahuali-oss/surrealdb" "${HOME}/.nahuali-oss/qdrant"

container_ready() {
  local service="$1"
  local status

  status="$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "nahuali-oss-${service}" 2>/dev/null || true)"
  [[ "$status" == "healthy" || "$status" == "running" ]]
}

qdrant_nofile_ready() {
  local limits

  limits="$(
    docker inspect \
      --format='{{range .HostConfig.Ulimits}}{{if eq .Name "nofile"}}{{.Soft}}:{{.Hard}}{{end}}{{end}}' \
      nahuali-oss-qdrant 2>/dev/null || true
  )"
  [[ "$limits" == "65535:65535" ]]
}

if ! container_ready surrealdb; then
  docker compose up -d surrealdb >/dev/null
fi

if ! container_ready qdrant; then
  docker compose up -d qdrant >/dev/null
elif ! qdrant_nofile_ready; then
  docker compose up -d --force-recreate qdrant >/dev/null
fi

for service in surrealdb qdrant; do
  for attempt in {1..90}; do
    status="$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "nahuali-oss-${service}" 2>/dev/null || true)"
    if [[ "$status" == "healthy" || "$status" == "running" ]]; then
      break
    fi
    if [[ "$attempt" -eq 90 ]]; then
      echo "service ${service} did not become healthy; last status: ${status:-missing}" >&2
      docker compose ps >&2
      exit 1
    fi
    sleep 1
  done
done

echo "dev database stack is ready"
