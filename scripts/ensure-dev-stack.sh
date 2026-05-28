#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SURREAL_CONTAINER="nahual-mictlan-surrealdb"
QDRANT_CONTAINER="nahual-tonalli-qdrant"
LEGACY_SURREAL_CONTAINER="nahuali-oss-surrealdb"
LEGACY_QDRANT_CONTAINER="nahuali-oss-qdrant"

mkdir -p "${HOME}/.nahual-rust/mictlan-surrealdb" "${HOME}/.nahual-rust/tonalli-qdrant"

container_name() {
  local service="$1"

  case "$service" in
    surrealdb) printf '%s\n' "$SURREAL_CONTAINER" ;;
    qdrant) printf '%s\n' "$QDRANT_CONTAINER" ;;
    *) echo "unknown service: $service" >&2; exit 1 ;;
  esac
}

stop_legacy_container() {
  local container="$1"
  local state

  state="$(docker inspect --format='{{.State.Status}}' "$container" 2>/dev/null || true)"
  if [[ "$state" == "running" ]]; then
    docker stop "$container" >/dev/null
  fi
}

stop_legacy_container "$LEGACY_SURREAL_CONTAINER"
stop_legacy_container "$LEGACY_QDRANT_CONTAINER"

container_ready() {
  local service="$1"
  local container
  local status

  container="$(container_name "$service")"
  status="$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container" 2>/dev/null || true)"
  [[ "$status" == "healthy" || "$status" == "running" ]]
}

qdrant_nofile_ready() {
  local limits

  limits="$(
    docker inspect \
      --format='{{range .HostConfig.Ulimits}}{{if eq .Name "nofile"}}{{.Soft}}:{{.Hard}}{{end}}{{end}}' \
      "$QDRANT_CONTAINER" 2>/dev/null || true
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
    container="$(container_name "$service")"
    status="$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container" 2>/dev/null || true)"
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
