#!/usr/bin/env bash
# Verify Bifrost is reachable from Arathyll's Caddy on arathyll_edge.
# Run on the VPS after any bifrost compose up/recreate.
set -euo pipefail

CADDY="${CADDY_CONTAINER:-arathyll-staging-caddy-1}"
fail=0

if ! docker inspect "$CADDY" >/dev/null 2>&1; then
  echo "FAIL: caddy container not found ($CADDY)"
  exit 1
fi

echo "Checking aliases from $CADDY …"

check() {
  local host="$1" url="$2"
  if docker exec "$CADDY" wget -qO- --timeout=3 "$url" >/dev/null 2>&1; then
    echo "  OK  $host"
  else
    echo "  FAIL $host (Caddy cannot reach $url on arathyll_edge)"
    echo "       Fix: cd /usr/deploy/bifrost/deploy && docker compose up -d"
    echo "       (requires COMPOSE_FILE=…edge.yml in deploy/.env)"
    fail=1
  fi
}

check bifrost_web "http://bifrost_web:8080/"
check bifrost_signal "http://bifrost_signal:8787/healthz"

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi

echo "Edge aliases OK."
