#!/bin/sh
# Writes the runtime configuration the app reads before it renders, and points
# nginx at the backend.
#
# nginx's own entrypoint runs everything in /docker-entrypoint.d before starting
# the server, which is why this is a plain script rather than an ENTRYPOINT.
set -eu

API_BASE_URL="${API_BASE_URL:-/api/v1}"
API_UPSTREAM="${API_UPSTREAM:-http://backend:8000}"

cat > /usr/share/nginx/html/config.js <<EOF
// Generated at container start-up. Edits here are lost on the next restart;
// set API_BASE_URL on the container instead.
window.__LAB_INVENTORY_CONFIG__ = { apiBaseUrl: "${API_BASE_URL}" };
EOF

# The upstream is only substituted into the proxy block, so a deployment that
# serves the API from another host can leave it at its default and simply set an
# absolute API_BASE_URL.
sed -i "s|__API_UPSTREAM__|${API_UPSTREAM}|g" /etc/nginx/conf.d/default.conf

echo "lab-inventory: apiBaseUrl=${API_BASE_URL} upstream=${API_UPSTREAM}"
