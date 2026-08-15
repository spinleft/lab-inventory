# Build context is the `backend` directory.
#
# Compiling is split so that a source-only change does not rebuild the
# dependency tree: cargo-chef turns the manifests into a recipe, and the layer
# that cooks it is cached until a dependency actually changes.
FROM rust:1-slim-bookworm AS chef
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends lld clang pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# `SQLX_OFFLINE` makes the query macros read `.sqlx/` instead of connecting to a
# database, which is the only way this can build without one.
ENV SQLX_OFFLINE=true
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin lab-inventory --bin lab-inventory-admin

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 lab-inventory

COPY --from=builder /app/target/release/lab-inventory /usr/local/bin/lab-inventory
COPY --from=builder /app/target/release/lab-inventory-admin /usr/local/bin/lab-inventory-admin
# Read relative to the working directory at run time, so it is copied rather
# than compiled in. `migrations` is embedded in the binary and shipped only so
# sqlx-cli can be pointed at it during maintenance.
COPY configuration configuration
COPY migrations migrations

# Attachments land here. Declared as a volume so an operator who forgets to
# mount one still keeps uploads across a container restart.
RUN mkdir -p /app/storage/attachments && chown -R lab-inventory:lab-inventory /app
VOLUME /app/storage
USER lab-inventory

ENV APP_ENVIRONMENT=production
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:8000/api/v1/health_check || exit 1
# CMD rather than ENTRYPOINT: the image ships two binaries, and an entrypoint
# would turn `docker compose run backend lab-inventory-admin …` into arguments
# handed to the *server* — which ignores them and starts serving instead.
CMD ["lab-inventory"]
