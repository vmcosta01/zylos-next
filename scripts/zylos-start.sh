#!/usr/bin/env bash
# Sobe gateway (127.0.0.1:3000, com token Hermes) e cockpit (3001). Uso: scripts/zylos-start.sh
cd "$(dirname "$0")/.." || exit 1
export ZYLOS_HERMES_TOKEN="$(cat .zylos-hermes-token)"
./target/debug/zylos-gateway.exe > gateway.log 2>&1 &
echo $! > .gateway.pid
(cd apps/zylos-cockpit && trunk serve --port 3001 --proxy-backend=http://127.0.0.1:3000/api/ --proxy-rewrite=/api/ > ../../trunk.log 2>&1 &)
echo "gateway: http://127.0.0.1:3000  cockpit: http://localhost:3001"
