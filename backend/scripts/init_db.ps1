# init_db.ps1
# 脚本作用: 初始化 PostgreSQL 数据库环境

# 启用详细输出
$VerbosePreference = "Continue"

# 启用错误中断
$ErrorActionPreference = "Stop"

# 检查是否安装了 sqlx
if (-not (Get-Command sqlx -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine("Error: sqlx is not installed.")
    [Console]::Error.WriteLine("Use:")
    [Console]::Error.WriteLine("    cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres")
    [Console]::Error.WriteLine("to install it.")
    exit 1
}

# 设置默认环境变量（如果未设置）
$env:DB_PORT = if ($env:DB_PORT) { $env:DB_PORT } else { "5432" }
$env:SUPERUSER = if ($env:SUPERUSER) { $env:SUPERUSER } else { "postgres" }
$env:SUPERUSER_PWD = if ($env:SUPERUSER_PWD) { $env:SUPERUSER_PWD } else { "password" }
$env:APP_USER = if ($env:APP_USER) { $env:APP_USER } else { "app" }
$env:APP_USER_PWD = if ($env:APP_USER_PWD) { $env:APP_USER_PWD } else { "secret" }
$env:APP_DB_NAME = if ($env:APP_DB_NAME) { $env:APP_DB_NAME } else { "inventory" }

# 如果未设置 SKIP_DOCKER，则启动 Docker 容器
if (-not $env:SKIP_DOCKER) {
    # 检查是否已有名为 postgres 的 Docker 容器在运行
    $runningPostgresContainers = docker ps --filter "name=postgres" --format "{{.ID}}"
    if ($runningPostgresContainers) {
        [Console]::Error.WriteLine("There is a postgres container already running. Kill it with:")
        [Console]::Error.WriteLine("    docker kill $runningPostgresContainers")
        exit 1
    }

    # 生成一个唯一的容器名称
    $timestamp = [int][double]::Parse((Get-Date -UFormat %s))
    $containerName = "postgres_$timestamp"

    # 启动 PostgreSQL Docker 容器
    docker run `
        --env POSTGRES_USER=$env:SUPERUSER `
        --env POSTGRES_PASSWORD=$env:SUPERUSER_PWD `
        --health-cmd="pg_isready -U $env:SUPERUSER || exit 1" `
        --health-interval=1s `
        --health-timeout=5s `
        --health-retries=5 `
        --publish "$env:DB_PORT`:5432" `
        --detach `
        --name "$containerName" `
        postgres -N 1000

    # 等待 PostgreSQL 容器变为健康状态
    Write-Host "Waiting for PostgreSQL to become healthy..."
    while ($true) {
        Start-Sleep -Seconds 1
        $healthStatus = docker inspect -f "{{.State.Health.Status}}" $containerName 2>$null
        if ($healthStatus -eq "healthy") {
            break
        }
        Write-Host "PostgreSQL is still unavailable - sleeping"
    }

    # 创建应用用户
    $createUserQuery = "CREATE USER $env:APP_USER WITH PASSWORD '$env:APP_USER_PWD';"
    docker exec $containerName psql -U $env:SUPERUSER -c $createUserQuery

    # 授予应用用户创建数据库的权限
    $grantPrivilegesQuery = "ALTER USER $env:APP_USER CREATEDB;"
    docker exec $containerName psql -U $env:SUPERUSER -c $grantPrivilegesQuery
}

Write-Host "PostgreSQL is up and running on port $env:DB_PORT - running migrations now!"

# 设置数据库连接字符串（移除密码周围的引号）
$databaseUrl = "postgres://$env:APP_USER`:$env:APP_USER_PWD@localhost:$env:DB_PORT/$env:APP_DB_NAME"
$env:DATABASE_URL = $databaseUrl

# 创建数据库
sqlx database create

# 运行数据库迁移
sqlx migrate run

Write-Host "PostgreSQL has been migrated, ready to go!"
