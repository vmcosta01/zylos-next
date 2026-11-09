#!/usr/bin/env bash
# Para gateway e cockpit (trunk).
for n in zylos-gateway.exe trunk.exe; do taskkill /IM $n /F 2>/dev/null; done
rm -f "$(dirname "$0")/../.gateway.pid"
