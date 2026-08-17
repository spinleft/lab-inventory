# 生产部署

本文覆盖用 Docker Compose 部署 Lab Inventory 的完整流程。想先在本地随便试试,看 [快速上手](quickstart.md);所有配置项的含义见 [配置参考](configuration.md)。

## 目录

- [部署形态](#部署形态)
- [环境要求](#环境要求)
- [一体化部署](#一体化部署)
- [配置 HTTPS](#配置-https)
- [使用外部数据库](#使用外部数据库)
- [不用 Docker 的部署方式](#不用-docker-的部署方式)
- [自己构建镜像](#自己构建镜像)
- [部署后检查清单](#部署后检查清单)

## 部署形态

系统由四个部分组成:

```
        浏览器 / 桌面端 / 安卓端
                  │
                  ▼
      ┌───────────────────────┐
      │  frontend (nginx)     │  静态页面 + 把 /api/ 转发给后端
      └───────────┬───────────┘
                  │
      ┌───────────▼───────────┐
      │  backend (actix-web)  │  REST API,附件存本地卷
      └─────┬───────────┬─────┘
            │           │
     ┌──────▼─────┐ ┌───▼──────┐
     │ PostgreSQL │ │  Redis   │  数据          会话
     └────────────┘ └──────────┘
```

前端容器里的 nginx 同时负责发静态页面和反向代理 API,所以浏览器只跟一个源打交道,不涉及跨域,会话 Cookie 也不需要放宽 SameSite。

有两个 Compose 文件:

| 文件 | 适用场景 |
| --- | --- |
| `docker-compose.yml` | **一体化**。Postgres、Redis 也由 Compose 拉起,单机部署最省事 |
| `docker-compose.external.yml` | 只跑应用容器,数据库和 Redis 用你已有的 |

## 环境要求

- Docker 24 及以上,带 Compose v2(`docker compose` 而非 `docker-compose`)
- x86_64(官方镜像只发 amd64;ARM 机器见 [自己构建镜像](#自己构建镜像))
- 2 GB 内存起步,磁盘按附件量准备
- 一个能从用户网络访问到的域名或 IP

## 一体化部署

### 1. 取得代码

只需要 Compose 文件和 `.env.example`,镜像从 GHCR 拉:

```bash
git clone --depth 1 --branch v0.1.0 https://github.com/spinleft/lab-inventory.git
cd lab-inventory
```

### 2. 写配置

```bash
cp .env.example .env
```

编辑 `.env`。四个必填项:

```dotenv
# 用户在浏览器里访问的完整地址。二维码里的链接用它生成,填错会导致扫码打不开。
PUBLIC_URL=https://inventory.example.com

# 会话签名密钥,至少 64 位随机串
HMAC_SECRET=<openssl rand -base64 48 的输出>

# 数据库口令,首次启动时创建,之后不要改
POSTGRES_PASSWORD=<另一个随机串>

# root 账号的初始密码,至少 8 位
INITIAL_ROOT_PASSWORD=<你要用来第一次登录的密码>
```

生成随机串:

```bash
openssl rand -base64 48
```

> **纯 HTTP 的内网部署**:再加一行 `COOKIE_SECURE=false`。默认的 `true` 会让浏览器只在 HTTPS 下回传会话 Cookie,在 HTTP 下的表现是"登录成功后立刻被踢回登录页"。

### 3. 启动

```bash
docker compose up -d
docker compose logs -f backend
```

后端第一次启动时会自己建表(执行内嵌的迁移脚本),并把 `root` 的密码设成 `INITIAL_ROOT_PASSWORD`。日志里看到这两行就说明就绪了:

```
Database schema is up to date.
The seeded root password was replaced with the configured initial password.
```

### 4. 首次登录

打开 `PUBLIC_URL`,用 `root` + `INITIAL_ROOT_PASSWORD` 登录,然后:

1. **改掉 root 密码**(右上角 → 个人设置 → 修改密码)。改完后 `.env` 里的 `INITIAL_ROOT_PASSWORD` 就不再起作用了,它只对"还是出厂密码"的账号生效。
2. **建实验室**:管理 → 实验室。系统自带一个叫"默认实验室"的空实验室,可以改名直接用,也可以新建。
3. **建管理员**:管理 → 用户,给实验室建一个 `lab_admin`,之后的日常维护用它,别拿 root 当日常账号。
4. 按需建位置树、计量单位、资产分类。

### 关于 root 出厂密码的保护

迁移脚本里种下的 `root` 密码是公开在本仓库源码里的。生产配置(`APP_ENVIRONMENT=production`)因此打开了一道保险:**只要 root 还用着这个出厂密码,服务就拒绝启动**,日志里会写清楚怎么处理。

正常路径是靠 `INITIAL_ROOT_PASSWORD` 自动完成替换。如果忘了配、服务起不来,可以直接改:

```bash
docker compose run --rm \
  -e LAB_INVENTORY_PASSWORD='你的新密码' \
  backend lab-inventory-admin set-password root
```

这个命令也可以用来重置任何用户的密码(把 `root` 换成对应用户名),忘记密码时用得上。

## 配置 HTTPS

前端容器只监听 HTTP 的 80 端口,映射到宿主机的 `HTTP_PORT`(默认 8080)。HTTPS 由前面的反向代理负责。仓库提供两种接法:叠加一个 Caddy 容器(推荐,证书全自动),或在你已有的宿主机反向代理前加一段配置。

### 用 Caddy 容器自动签发证书(推荐)

仓库根目录的 `Caddyfile` 和 `docker-compose.caddy.yml` 在前面任何一个 Compose 文件之上叠加一个 Caddy 容器,监听 80/443,自动向 Let's Encrypt / ZeroSSL 申请并续期证书,再把请求转发给 `frontend` 容器。

1. 在 `.env` 里填好 `DOMAIN` 和 `ACME_EMAIL`,并确认 `PUBLIC_URL` 是 `https://<DOMAIN>`、`COOKIE_SECURE=true`:

   ```dotenv
   PUBLIC_URL=https://inventory.example.com
   DOMAIN=inventory.example.com
   ACME_EMAIL=you@example.com
   ```

2. 确保 `DOMAIN` 已解析到本机公网 IP,80/443 端口能从公网访问(防火墙/安全组放行),并且这两个端口没被别的进程占用。

3. 叠加启动:

   ```bash
   docker compose -f docker-compose.yml -f docker-compose.caddy.yml up -d
   ```

   用外部数据库时,把第一个 `-f` 换成 `docker-compose.external.yml`。

4. 首次启动 Caddy 自动完成证书申请。验证:

   ```bash
   curl https://inventory.example.com/api/v1/health_check
   ```

   证书存在 `caddy-data` 卷里,重启容器不会重新申请,Caddy 会在到期前自动续期。

> **没有域名 / 只有内网 IP**:公网证书机构无法给内网地址签发证书,自动 HTTPS 用不了。要么继续纯 HTTP(记得 `COOKIE_SECURE=false`),要么在 `Caddyfile` 里把站点地址换成 `tls internal` 自签证书(浏览器会提示不受信任)。

### 用宿主机上的 Caddy

不想把 Caddy 放进 Compose,也可以在宿主机上装一个,指向前端映射出来的端口:

```caddy
inventory.example.com {
    reverse_proxy localhost:8080
}
```

### 用宿主机上的 Nginx

```nginx
server {
    listen 443 ssl http2;
    server_name inventory.example.com;

    ssl_certificate     /etc/letsencrypt/live/inventory.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/inventory.example.com/privkey.pem;

    # 附件上传要穿过这一层,限制得比后端的 50 MiB 宽一点
    client_max_body_size 64m;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        # 附件下载是流式的,别在这里缓冲
        proxy_buffering off;
    }
}

server {
    listen 80;
    server_name inventory.example.com;
    return 301 https://$host$request_uri;
}
```

配好之后确认 `.env` 里的 `PUBLIC_URL` 是 `https://` 开头、`COOKIE_SECURE` 保持 `true`,然后 `docker compose up -d` 让后端读到新值。

## 使用外部数据库

已有 Postgres/Redis 运维体系,或者用云托管数据库时,改用 `docker-compose.external.yml`。

在 `.env` 里补上连接信息:

```dotenv
DATABASE_HOST=db.internal
DATABASE_PORT=5432
DATABASE_USERNAME=lab_inventory
DATABASE_PASSWORD=<口令>
DATABASE_NAME=lab_inventory
# 默认要求 TLS。数据库不支持时才改成 false
DATABASE_REQUIRE_SSL=true

REDIS_URI=redis://redis.internal:6379
```

数据库需要提前建好,且 `DATABASE_USERNAME` 对它有建表权限——表结构由后端启动时自己创建:

```sql
CREATE DATABASE lab_inventory;
```

启动:

```bash
docker compose -f docker-compose.external.yml up -d
```

> **Redis 的数据是可丢的**:里面只有会话。丢了的后果是所有人需要重新登录,不影响业务数据。因此 Redis 不需要做备份,但要保证可用——它挂了会导致无法登录。

## 不用 Docker 的部署方式

Release 页面提供 `lab-inventory-server-<版本>-x86_64-linux.tar.gz`,里面是后端二进制、管理命令、配置文件和迁移脚本;`lab-inventory-web-<版本>.tar.gz` 是前端静态文件。

```bash
tar -xzf lab-inventory-server-0.1.0-x86_64-linux.tar.gz
cd lab-inventory-server-0.1.0-x86_64-linux

export APP_ENVIRONMENT=production
export APP_DATABASE__HOST=127.0.0.1
export APP_DATABASE__PASSWORD=...
export APP_APPLICATION__HMAC_SECRET=...
export APP_APPLICATION__BASE_URL=https://inventory.example.com
export APP_APPLICATION__PUBLIC_WEB_URL=https://inventory.example.com
export APP_APPLICATION__INITIAL_ROOT_PASSWORD=...

./lab-inventory
```

注意两点:

- 二进制在**启动时的工作目录**下找 `configuration/`,所以必须在解压出来的目录里运行(systemd 里用 `WorkingDirectory=`)。
- 二进制是在 Ubuntu 22.04 上编译的,需要 glibc 2.35 或更新(Debian 12、Ubuntu 22.04+、RHEL 9+ 都满足)。更老的系统请用 Docker 镜像。

前端静态包解压到 nginx 的目录下,再把 `/api/` 转发给后端。后端地址通过 `config.js` 告诉前端:

```javascript
// dist/config.js
window.__LAB_INVENTORY_CONFIG__ = { apiBaseUrl: "/api/v1" };
```

填相对路径 `/api/v1` 表示"跟页面同源",由 nginx 转发;后端在另一个域名时填完整 URL,同时要把那个域名加进后端的 `cors_allowed_origins`。

systemd 单元文件的例子见 [运维文档](operations.md#用-systemd-托管)。

## 自己构建镜像

官方镜像只发 `linux/amd64`。ARM 机器(树莓派、Apple Silicon、ARM 云主机)自己构建:

```bash
docker build -t lab-inventory-backend:local ./backend
docker build -t lab-inventory-frontend:local ./frontend
```

然后在 `docker-compose.yml` 里把 `image:` 换成本地 tag,或者把注释掉的 `build:` 段打开。

后端镜像的构建**不需要数据库**:编译期的 SQL 校验读的是仓库里 `backend/.sqlx/` 的查询缓存。

## 部署后检查清单

- [ ] `curl https://<域名>/api/v1/health_check` 返回 `{"status":"ok"}`
- [ ] root 密码已经在界面上改过,不再是 `.env` 里那个
- [ ] `.env` 的权限是 `600`,且没有被提交进 git
- [ ] `HMAC_SECRET` 是随机生成的,不是示例值
- [ ] HTTPS 可用,`COOKIE_SECURE=true`
- [ ] 数据库备份任务已经配好(见 [运维文档](operations.md#备份))
- [ ] 附件卷 `attachments` 在备份范围内
- [ ] 日常操作用的是 `lab_admin` 账号而不是 root
