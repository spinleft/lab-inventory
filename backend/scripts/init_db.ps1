$ErrorActionPreference = "Stop"

if (-not (Get-Command sqlx -ErrorAction SilentlyContinue)) {
    Write-Error "sqlx is not installed. Run: cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres"
}

$DB_PORT = if ($env:DB_PORT) { $env:DB_PORT } else { "5432" }
$SUPERUSER = if ($env:SUPERUSER) { $env:SUPERUSER } else { "postgres" }
$SUPERUSER_PWD = if ($env:SUPERUSER_PWD) { $env:SUPERUSER_PWD } else { "password" }
$APP_USER = if ($env:APP_USER) { $env:APP_USER } else { "app" }
$APP_USER_PWD = if ($env:APP_USER_PWD) { $env:APP_USER_PWD } else { "secret" }
$APP_DB_NAME = if ($env:APP_DB_NAME) { $env:APP_DB_NAME } else { "lab_inventory" }

if (-not $env:SKIP_DOCKER) {
    $running = docker ps --filter "name=postgres" --format "{{.ID}}"
    if ($running) {
        Write-Error "there is a postgres container already running, kill it with: docker kill $running"
    }

    $containerName = "postgres_$(Get-Date -Format 'yyyyMMddHHmmss')"
    docker run `
        --env POSTGRES_USER=$SUPERUSER `
        --env POSTGRES_PASSWORD=$SUPERUSER_PWD `
        --health-cmd="pg_isready -U $SUPERUSER || exit 1" `
        --health-interval=1s `
        --health-timeout=5s `
        --health-retries=5 `
        --publish "$DB_PORT`:5432" `
        --detach `
        --name "$containerName" `
        postgres -N 1000

    do {
        Write-Host "Postgres is still unavailable - sleeping"
        Start-Sleep -Seconds 1
        $status = docker inspect -f "{{.State.Health.Status}}" $containerName
    } until ($status -eq "healthy")

    docker exec -it "$containerName" psql -U "$SUPERUSER" -c "CREATE USER $APP_USER WITH PASSWORD '$APP_USER_PWD';"
    docker exec -it "$containerName" psql -U "$SUPERUSER" -c "ALTER USER $APP_USER CREATEDB;"
}

$env:DATABASE_URL = "postgres://$APP_USER`:$APP_USER_PWD@localhost:$DB_PORT/$APP_DB_NAME"
sqlx database create
sqlx migrate run

Write-Host "Postgres has been migrated, ready to go!"
