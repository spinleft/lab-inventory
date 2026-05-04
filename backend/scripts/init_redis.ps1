$ErrorActionPreference = "Stop"

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Error "docker is not installed. Install Docker Desktop, then retry this script."
}

docker info *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker daemon is not running or is not reachable. Start Docker Desktop, wait until it is ready, then retry this script."
}

$running = docker ps --filter "name=redis" --format "{{.ID}}"
if ($running) {
    Write-Error "there is a redis container already running, kill it with: docker kill $running"
}

$containerName = "redis_$(Get-Date -Format 'yyyyMMddHHmmss')"
docker run `
    -p "6379:6379" `
    -d `
    --name "$containerName" `
    redis:7

Write-Host "Redis is ready to go!"
