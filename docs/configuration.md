# 配置参考

后端的所有配置项、默认值,以及怎么用环境变量覆盖它们。

## 配置是怎么加载的

后端启动时按顺序读三层,后面的覆盖前面的:

1. `configuration/base.yaml` —— 所有环境共用的默认值
2. `configuration/<环境>.yaml` —— `local.yaml` 或 `production.yaml`,由环境变量 `APP_ENVIRONMENT` 决定,默认 `local`
3. 以 `APP_` 开头的环境变量

配置文件是从**启动时的工作目录**下的 `configuration/` 读的,不是从二进制所在目录。Docker 镜像里工作目录是 `/app`,配置在 `/app/configuration`。

### 环境变量的命名规则

`APP_` 前缀,层级之间用**两个下划线**分隔,全大写:

| 配置项 | 环境变量 |
| --- | --- |
| `application.port` | `APP_APPLICATION__PORT` |
| `database.password` | `APP_DATABASE__PASSWORD` |
| `federation.public_base_url` | `APP_FEDERATION__PUBLIC_BASE_URL` |
| `redis_uri`(顶层) | `APP_REDIS_URI` |

列表类型(`cors_allowed_origins`、`allowed_remote_hosts`)用逗号分隔:

```bash
APP_APPLICATION__CORS_ALLOWED_ORIGINS=http://127.0.0.1:5173,https://inventory.example.com
```

## application

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `host` | `0.0.0.0` | 监听地址。容器里保持 `0.0.0.0` |
| `port` | `8000` | 监听端口 |
| `base_url` | `http://127.0.0.1:8000` | 本服务自己的外部地址 |
| `public_web_url` | `http://127.0.0.1:5173` | **前端页面**的地址。二维码里的链接指向它,所以必须填用户实际访问的地址,而不是 API 的地址。留空时回落到 `federation.public_base_url` |
| `hmac_secret` | 示例值 | 会话 Cookie 和访客注册码的签名密钥。**必须改**,至少 64 位随机串 |
| `cookie_secure` | `false`(生产 `true`) | 会话 Cookie 是否只在 HTTPS 下发送。纯 HTTP 部署必须设成 `false`,否则登录后会立刻掉线 |
| `enable_federation` | `true` | 是否启用联邦功能 |
| `cors_allowed_origins` | 见 `base.yaml` | 允许跨域携带 Cookie 的来源列表。同源部署留空即可 |
| `initial_root_password` | 无 | root 的初始密码。**只在 root 还是出厂密码时生效**,之后每次启动都会被忽略,不会覆盖已经改过的密码 |
| `require_root_password_rotation` | `false`(生产 `true`) | root 还是出厂密码时是否拒绝启动 |

### 关于 `hmac_secret`

生成方式:

```bash
openssl rand -base64 48
```

它同时用于派生会话 Cookie 的加密密钥和访客注册码的签名。**换掉它会让所有已登录用户掉线**,已经发出去但还没用的访客注册码也会失效。

### 关于 `public_web_url`

标签上的二维码里存的是这样一条链接:

```
<public_web_url>/scan?v=1&n=<节点 id>&l=<实验室 id>&t=item&i=<条目 id>
```

用手机的相机直接扫,打开的就是这个地址,所以它必须是**用户实际能打开的前端地址**。填成 API 地址或 `127.0.0.1` 的后果是标签打出来了但扫不开——而标签已经贴到实物上了,返工成本很高。**部署后先打一张试扫一下**。

(在应用内用扫码页扫描时走的是另一条路径:只读 `?` 后面的参数,不关心域名。这样才能扫开联邦对端实验室打的标签。)

## database

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `host` | `127.0.0.1` | Postgres 地址 |
| `port` | `5432` | 端口 |
| `username` | `postgres` | 用户名 |
| `password` | `password` | 口令。**必须改** |
| `database_name` | `lab_inventory` | 库名 |
| `require_ssl` | `false`(生产 `true`) | 是否要求 TLS 连接。同一台机器/同一个 Docker 网络内可以关掉 |
| `run_migrations` | `false`(生产 `true`) | 启动时是否自动执行未应用的迁移 |

`run_migrations` 打开后,升级部署就只是换个镜像 tag 再重启。迁移过程带 Postgres 咨询锁,多个实例同时启动也不会打架。本地开发关掉它,是为了让忘记写迁移这件事及早暴露,而不是被自动补上。

## redis_uri

会话存储。顶层配置项,环境变量是 `APP_REDIS_URI`。

```yaml
redis_uri: redis://127.0.0.1:6379
```

里面只有会话数据,丢了的后果是所有人重新登录,**不需要备份**;但它挂了会导致无法登录,要保证可用。带口令的写法:`redis://:<口令>@host:6379`。

## file_storage

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `backend` | `local` | 存储后端。目前只支持 `local` |
| `local_root` | `./storage/attachments` | 附件目录。Docker 镜像里是 `/app/storage/attachments`,已声明为卷 |
| `max_file_size_bytes` | `52428800`(50 MiB) | 单个附件大小上限 |
| `upload_token_ttl_minutes` | `60` | 上传凭证的有效期 |

调大 `max_file_size_bytes` 时,记得同步放宽前面每一层反向代理的 `client_max_body_size`,否则大文件会被代理挡在外面,报错却出现在浏览器端。

## federation

跨实例互查与借用。概念和操作流程见 [联邦文档](federation.md)。

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `enabled` | `true` | 总开关 |
| `public_base_url` | `http://127.0.0.1:8000` | 本实例对外的 API 地址,配对时告诉对方 |
| `require_https` | `true` | 是否只接受 HTTPS 的对端 |
| `allow_insecure_private_network` | `false`(生产 `false`) | 是否允许对端是私网地址。开发环境才打开 |
| `request_ttl_seconds` | `300` | 联邦请求签名的有效期,用于防重放 |
| `allowed_remote_hosts` | `[]` | 对端主机白名单。留空表示不额外限制,只按信任关系判断 |

## label_printing

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `allow_loopback` | `false` | 是否允许把打印机地址注册成回环地址 |

打印机地址是**服务端会去主动连接**的地址,所以注册权限只给实验室管理员,且默认禁止回环——否则一条打印机记录就能让服务器去连自己的其他端口。只有本地开发拿假打印机测试时才打开。

## 完整的环境变量清单

生产部署常用的一套:

```bash
APP_ENVIRONMENT=production

APP_DATABASE__HOST=postgres
APP_DATABASE__PORT=5432
APP_DATABASE__USERNAME=lab_inventory
APP_DATABASE__PASSWORD=<口令>
APP_DATABASE__DATABASE_NAME=lab_inventory
APP_DATABASE__REQUIRE_SSL=false

APP_REDIS_URI=redis://redis:6379

APP_APPLICATION__HMAC_SECRET=<随机串>
APP_APPLICATION__BASE_URL=https://inventory.example.com
APP_APPLICATION__PUBLIC_WEB_URL=https://inventory.example.com
APP_APPLICATION__INITIAL_ROOT_PASSWORD=<初始密码>
APP_APPLICATION__COOKIE_SECURE=true
APP_APPLICATION__CORS_ALLOWED_ORIGINS=

APP_FEDERATION__ENABLED=false
APP_FEDERATION__PUBLIC_BASE_URL=https://inventory.example.com

APP_LABEL_PRINTING__ALLOW_LOOPBACK=false
```

## 前端配置

前端的唯一配置项是后端 API 地址,按优先级从高到低:

1. **用户在界面里设置的**——"服务器设置"页面,存在浏览器 localStorage 里,只影响这一台设备的这个浏览器
2. **`config.js` 里的运行时配置**——Docker 镜像在启动时按 `API_BASE_URL` 环境变量生成
3. **构建时烘进去的 `VITE_DEFAULT_API_BASE_URL`**——桌面端和安卓端用这个
4. 都没有时回落到 `http://127.0.0.1:8000/api/v1`

`config.js` 的内容:

```javascript
window.__LAB_INVENTORY_CONFIG__ = { apiBaseUrl: "/api/v1" };
```

填相对路径(以 `/` 开头)表示"跟页面同源",由前面的 nginx 转发;填完整 URL 表示后端在别的地址,这时要把前端的域名加进后端的 `cors_allowed_origins`。

前端容器的环境变量:

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `API_BASE_URL` | `/api/v1` | 写进 `config.js` 的值 |
| `API_UPSTREAM` | `http://backend:8000` | nginx 把 `/api/` 转发到哪里 |
