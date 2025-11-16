$ErrorActionPreference = 'Stop'
# 可选：启用类似 bash `set -x` 的命令跟踪（在某些环境中可能不可用）
if (Get-Command Set-PSDebug -ErrorAction SilentlyContinue) { Set-PSDebug -Trace 1 }

# 如果有名为 redis 的容器在运行，打印如何停止它并退出
$runningContainer = docker ps --filter "name=redis" --format "{{.ID}}"
if (-not [string]::IsNullOrWhiteSpace($runningContainer)) {
    Write-Error "There is a redis container already running, kill it with`ndocker kill $runningContainer"
    exit 1
}

# 启动 Redis 容器
$containerName = "redis_$(Get-Date -Format 'yyyyMMddHHmmss')"
docker run `
  -p 6379:6379 `
  -d `
  --name $containerName `
  redis:7

Write-Host "Redis is ready to go!" -ForegroundColor Green

# 关闭命令跟踪（如果之前启用）
if (Get-Command Set-PSDebug -ErrorAction SilentlyContinue) { Set-PSDebug -Trace 0 }