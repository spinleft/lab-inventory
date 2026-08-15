# Build context is the `frontend` directory.
#
# The bundle is built without an API address baked in. `docker-entrypoint.sh`
# writes one into `config.js` when the container starts, so the same image can
# be pointed at any backend.
FROM node:22-bookworm-slim AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:1.31-alpine AS runtime
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY docker-entrypoint.sh /docker-entrypoint.d/40-lab-inventory-config.sh
RUN chmod +x /docker-entrypoint.d/40-lab-inventory-config.sh

# Where the app looks for the API. The default is same-origin, which this
# image's nginx reverse-proxies to `API_UPSTREAM`; point it at an absolute URL
# instead when the API is served from somewhere else.
ENV API_BASE_URL=/api/v1
ENV API_UPSTREAM=http://backend:8000

EXPOSE 80
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget --quiet --tries=1 --spider http://127.0.0.1/ || exit 1
