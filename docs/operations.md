# 运维

日常维护:备份、升级、排障。

## 备份

需要备份的只有两样:

| 内容 | 位置 | 丢了会怎样 |
| --- | --- | --- |
| PostgreSQL 数据库 | `postgres-data` 卷 | 全部业务数据没了 |
| 附件文件 | `attachments` 卷 | 说明书、照片、校准报告没了,数据库里的记录会指向不存在的文件 |

Redis 里只有会话,**不需要备份**。

`.env` 不算备份内容,但要单独妥善保管:里面的 `HMAC_SECRET` 换掉会让所有人掉线,`POSTGRES_PASSWORD` 丢了连不上自己的数据库。

### 手动备份

```bash
#!/usr/bin/env bash
set -euo pipefail

BACKUP_DIR=/var/backups/lab-inventory
STAMP=$(date +%Y%m%d-%H%M%S)
mkdir -p "$BACKUP_DIR"

# 数据库
docker compose exec -T postgres \
  pg_dump -U lab_inventory -Fc lab_inventory \
  > "$BACKUP_DIR/db-$STAMP.dump"

# 附件。用一个临时容器挂上卷再打包,不依赖卷在宿主机上的具体路径。
docker run --rm \
  -v lab-inventory_attachments:/data:ro \
  -v "$BACKUP_DIR":/backup \
  alpine tar -czf "/backup/attachments-$STAMP.tar.gz" -C /data .

# 只留最近 30 天
find "$BACKUP_DIR" -type f -mtime +30 -delete
```

存成 `/usr/local/bin/lab-inventory-backup.sh`,加执行权限,挂到 cron:

```cron
0 3 * * * /usr/local/bin/lab-inventory-backup.sh >> /var/log/lab-inventory-backup.log 2>&1
```

> 卷名前缀是 Compose 的项目名,本仓库的 `docker-compose.yml` 里写死成了 `lab-inventory`。用 `docker volume ls` 确认一下。

### 恢复

```bash
docker compose stop backend frontend

# 数据库:先清空再灌,--clean 会先删掉已有对象
docker compose exec -T postgres \
  pg_restore -U lab_inventory -d lab_inventory --clean --if-exists \
  < /var/backups/lab-inventory/db-20260816-030000.dump

# 附件
docker run --rm \
  -v lab-inventory_attachments:/data \
  -v /var/backups/lab-inventory:/backup:ro \
  alpine sh -c "rm -rf /data/* && tar -xzf /backup/attachments-20260816-030000.tar.gz -C /data"

docker compose start backend frontend
```

**备份要定期演练恢复。** 没验证过的备份等于没有备份。

## 升级

### Docker 部署

```bash
cd /path/to/lab-inventory

# 1. 先备份(见上)

# 2. 改 .env 里的版本号
sed -i 's/^LAB_INVENTORY_VERSION=.*/LAB_INVENTORY_VERSION=0.2.0/' .env

# 3. 拉新镜像并重启
docker compose pull
docker compose up -d

# 4. 看日志确认迁移执行完了
docker compose logs -f backend
```

生产配置下 `run_migrations` 是开着的,后端启动时会自己把数据库结构升上去,不需要额外操作。迁移带 Postgres 咨询锁,多实例同时启动也安全。

**升级前务必读一遍对应版本的 [CHANGELOG](../CHANGELOG.md)**,里面会写清楚有没有不兼容变更。

### 回滚

改回旧版本号再 `docker compose up -d` 即可——**前提是那次升级没有带数据库迁移**。带了迁移的话,旧版本的代码不认得新表结构,必须从备份恢复数据库。CHANGELOG 里会标注哪些版本含迁移。

## 用 systemd 托管

不走 Docker 时,把解压出来的服务端包放在 `/opt/lab-inventory`:

```ini
# /etc/systemd/system/lab-inventory.service
[Unit]
Description=Lab Inventory
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=lab-inventory
Group=lab-inventory
# 二进制在启动时的工作目录下找 configuration/,这行不能少
WorkingDirectory=/opt/lab-inventory
ExecStart=/opt/lab-inventory/lab-inventory
# 密钥放在只有本服务能读的文件里,不要直接写在这里
EnvironmentFile=/etc/lab-inventory/env
Restart=on-failure
RestartSec=5

# 除了附件目录之外,不需要写任何地方
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/lab-inventory/storage

[Install]
WantedBy=multi-user.target
```

```bash
sudo install -d -m 700 -o root -g root /etc/lab-inventory
sudo install -m 600 /dev/null /etc/lab-inventory/env
# 把 docs/configuration.md 里那份环境变量清单写进去
sudo systemctl daemon-reload
sudo systemctl enable --now lab-inventory
```

## 排障

### 启动报 `server does not support TLS`

完整错误长这样:

```
Error: Failed to apply database migrations.
Caused by:
    error occurred while attempting to establish a TLS connection:
    server does not support TLS
```

生产配置(`APP_ENVIRONMENT=production`)默认 `database.require_ssl: true`,要求用 TLS 连数据库;而你连的这个 Postgres 没开 TLS。**Postgres 官方镜像默认就是不开的**(`SHOW ssl` 返回 `off`,数据目录里没有服务端证书),所以自建数据库十有八九会撞上。

确认一下数据库那边:

```bash
docker compose exec postgres psql -U lab_inventory -t -c "SHOW ssl;"
```

返回 `off` 就是这个原因。怎么处理取决于数据库在哪:

**数据库和应用在同一台机器 / 同一个 Docker 网络** —— 流量不出宿主机,不需要 TLS,关掉即可:

```dotenv
APP_DATABASE__REQUIRE_SSL=false
```

仓库自带的 `docker-compose.yml` 已经这么设了,一体化部署不会遇到这个问题。用 `docker-compose.external.yml` 时对应的是 `.env` 里的 `DATABASE_REQUIRE_SSL=false`。

**数据库在另一台机器上** —— 这时候 TLS 是该有的,别关。给 Postgres 配上证书:

```conf
# postgresql.conf
ssl = on
ssl_cert_file = 'server.crt'
ssl_key_file = 'server.key'
```

托管数据库(RDS、云数据库等)通常本来就开着 TLS,保持 `true` 就行。

> `require_ssl: true` 对应 sqlx 的 `PgSslMode::Require`:**只加密,不校验服务端证书**。它挡得住旁路监听,挡不住中间人。真要防中间人,得把数据库放在可信网络里,或者在数据库前面加一层双向认证的代理。

### 登录后立刻退回登录页

会话 Cookie 没被浏览器保存。绝大多数情况是 `cookie_secure=true` 但访问用的是 HTTP。

- 走 HTTPS(推荐),或者
- 在 `.env` 里设 `COOKIE_SECURE=false` 再 `docker compose up -d`

也可能是 Redis 连不上——`docker compose logs backend` 里会有连接错误。

### 服务起不来,日志说 root 还是出厂密码

```
The `root` account still uses the password seeded by the migrations...
```

生产配置的保险起作用了:迁移脚本里种的那个 root 密码是公开在源码里的。按日志的提示做:

```bash
docker compose run --rm \
  -e LAB_INVENTORY_PASSWORD='新密码' \
  backend lab-inventory-admin set-password root
docker compose up -d
```

### 忘了密码

```bash
docker compose run --rm \
  -e LAB_INVENTORY_PASSWORD='新密码' \
  backend lab-inventory-admin set-password <用户名>
```

### 扫码打不开 / 打开的是错的地址

二维码里的地址来自 `application.public_web_url`。它错了,已经打出来的标签也就错了,只能重打。

```bash
docker compose exec backend env | grep PUBLIC_WEB_URL
```

改 `.env` 里的 `PUBLIC_URL`,`docker compose up -d`,然后重打受影响的标签。

### 上传附件报错

大概率是某一层代理的请求体大小限制没放开。后端默认允许 50 MiB,但请求要先穿过前端容器的 nginx(已设成 64m)和你自己那层反向代理(默认往往只有 1m)。

在你的反向代理配置里加:

```nginx
client_max_body_size 64m;
```

### 打印机连不上

- 打印机和服务器要在同一个网络里,服务器要能连上打印机的 9100 端口
- 打印机地址不能是回环地址(`127.0.0.1`),服务端会拒绝
- 用界面上的"检测状态"看具体报错

细节见 [标签打印文档](label-printing.md)。

### 看日志

```bash
docker compose logs -f backend     # 后端
docker compose logs -f frontend    # nginx 访问日志
docker compose logs -f postgres
```

后端日志是 JSON 格式(bunyan),字段多的时候用 `jq` 过:

```bash
docker compose logs --no-log-prefix backend | jq 'select(.level >= 50)'
```

### 健康检查

```bash
curl http://localhost:8080/api/v1/health_check
# {"status":"ok"}
```

它只说明进程活着并且在响应,不检查数据库连接。数据库出问题会在业务接口上表现为 500,日志里能看到。

## 常用命令速查

```bash
docker compose ps                    # 各容器状态
docker compose restart backend       # 只重启后端
docker compose down                  # 停掉(卷保留,数据不丢)
docker compose down -v               # 停掉并删卷 —— 数据全没,慎用

# 进数据库
docker compose exec postgres psql -U lab_inventory -d lab_inventory

# 看当前部署的版本
docker compose exec backend lab-inventory-admin version
```
