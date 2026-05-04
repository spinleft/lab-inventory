#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v docker)" ]; then
  echo >&2 "Error: docker is not installed."
  echo >&2 "Install Docker Desktop, then retry this script."
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo >&2 "Error: Docker daemon is not running or is not reachable."
  echo >&2 "Start Docker Desktop, wait until it is ready, then retry this script."
  exit 1
fi

RUNNING_CONTAINER=$(docker ps --filter 'name=redis' --format '{{.ID}}')
if [[ -n $RUNNING_CONTAINER ]]; then
  echo >&2 "there is a redis container already running, kill it with"
  echo >&2 "    docker kill ${RUNNING_CONTAINER}"
  exit 1
fi

docker run \
  -p "6379:6379" \
  -d \
  --name "redis_$(date '+%s')" \
  redis:7

>&2 echo "Redis is ready to go!"
