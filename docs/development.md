# 开发

本地开发环境、测试、代码约定。

## 环境准备

| 工具 | 版本 | 用途 |
| --- | --- | --- |
| Rust | 1.85+(edition 2024) | 后端 |
| Node.js | 22+ | 前端 |
| Docker | 任意近期版本 | 跑 Postgres 和 Redis |
| sqlx-cli | 0.8.x | 数据库迁移、查询缓存 |

```bash
cargo install sqlx-cli --version 0.8.6 --no-default-features --features rustls,postgres --locked
```

版本要跟 `backend/Cargo.toml` 里的 sqlx 大版本对上,否则生成的 `.sqlx` 缓存格式不兼容。

## 起依赖

```bash
cd backend
cp .env.example .env      # sqlx 的编译期校验要从这里读连接串
./scripts/init_db.sh      # 起 Postgres 容器、建库、跑迁移
./scripts/init_redis.sh   # 起 Redis 容器
```

`backend/.env` 不在版本库里。没有它的话,`cargo build` 会因为连不上数据库而没法做编译期的 SQL 校验;临时绕过可以用 `SQLX_OFFLINE=true`,那样读的是仓库里的 `.sqlx` 查询缓存。

Windows 上用 PowerShell 版本:`.\scripts\init_db.ps1`、`.\scripts\init_redis.ps1`。

容器已经在跑时,跳过起容器只跑迁移:

```bash
SKIP_DOCKER=true ./scripts/init_db.sh
```

## 跑起来

```bash
# 后端,监听 127.0.0.1:8000
cd backend
cargo run

# 前端,监听 127.0.0.1:5173
cd frontend
npm install
npm run dev
```

浏览器打开 <http://127.0.0.1:5173>。开发环境默认后端地址是 `http://127.0.0.1:8000/api/v1`,前端会自动带上。

默认账号是 `root`,密码是迁移脚本里种的那个(公开在源码里,见 `backend/migrations/20260430000000_initial_schema.sql`)。本地环境不强制改;生产配置会拒绝用出厂密码启动。

## 测试

```bash
# 后端。需要 Postgres 在跑:每个用例会自己建一个独立的库,跑完删掉
cd backend
cargo test

# 前端单元/组件测试
cd frontend
npm run test

# 端到端测试(Playwright)。用例自己拦截 API 请求,不需要后端
npm run test:e2e
```

后端单个用例:

```bash
cargo test --test api bootstrap                 # 一个文件
cargo test --test api the_initial_root_password # 一个用例
TEST_LOG=1 cargo test --test api bootstrap      # 带日志
```

## 提交前

CI 会卡这几项,本地先跑一遍能省一轮往返:

```bash
cd backend
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test

cd ../frontend
npm run typecheck
npm run test
npm run build
```

## 数据库

### 加一个迁移

```bash
cd backend
sqlx migrate add <描述性名字>
```

会在 `migrations/` 下生成一个带时间戳的空文件。写完之后:

```bash
sqlx migrate run
```

**已经提交进仓库的迁移不要再改内容。** sqlx 记录每个迁移的校验和,改了之后所有已有数据库都会拒绝继续迁移。要改就再加一个迁移。

### 查询缓存(`.sqlx`)

`sqlx::query!` 宏在编译期连数据库校验 SQL。但 Docker 构建和 CI 的 lint 任务都没有数据库,它们读的是提交进仓库的 `backend/.sqlx/`。

**改了任何 `query!` 里的 SQL,都要重新生成:**

```bash
cd backend
cargo sqlx prepare -- --all-targets
git add .sqlx
```

忘了这一步的话,CI 的"检查 .sqlx 是否为最新"任务会失败。

### 不用宏的查询

代码里同时用了 `sqlx::query!`(编译期校验)和 `sqlx::query`(运行时)。动态拼条件的地方用后者,其余优先用前者。

## 前端依赖锁文件

`frontend/package-lock.json` 里必须包含**所有平台**的原生二进制条目(rollup、esbuild、Tauri CLI 各自的平台包)。CI 在 Linux 和 macOS 上跑 `npm ci`,而 `npm ci` 只装锁文件里写着的东西——少了对应平台的条目就会报 `Cannot find module @rollup/rollup-linux-x64-gnu`([npm/cli#4828](https://github.com/npm/cli/issues/4828))。

麻烦之处在于:**在 Windows 上删掉锁文件重新 `npm install`,生成的锁文件只会包含 Windows 的条目**。所以要重新生成锁文件时,在 Linux 容器里做:

```bash
cd frontend
rm package-lock.json
# 用一个匿名卷把宿主机的 node_modules 挡住,否则 npm 会照着已装的
# (Windows 的)依赖树反推,又只写回 Windows 的条目
docker run --rm -v "$(pwd)":/w -v /w/node_modules -w /w node:22-bookworm-slim \
  npm install --package-lock-only
```

生成后确认一下:

```bash
node -e "const d=require('./package-lock.json');console.log(!!d.packages['node_modules/@rollup/rollup-linux-x64-gnu'])"
```

平常加删依赖走正常的 `npm install <包名>` 就行,它只会往锁文件里增删对应条目,不会把别的平台的条目清掉。

## 代码结构

```
backend/
  src/
    domain/          领域类型。每个文件一个,带 parse 校验,把无效值挡在类型系统外面
    routes/          按资源分目录,每个目录里:
                       create/get/list/update/delete.rs  各个 handler
                       model.rs                          请求/响应结构
                       queries.rs                        SQL
                       service.rs                        编排逻辑
    authentication/  登录、密码、中间件、访客注册
    access_control.rs  权限判定,集中在一处
    file_storage/    附件存储
    label_printing/  标签光栅化和打印机通信
    bootstrap.rs     启动时的迁移和 root 密码保护
    configuration.rs 配置结构
    startup.rs       路由注册和中间件装配
  migrations/        数据库迁移
  tests/api/         集成测试,按资源分文件

frontend/
  src/
    app/             路由、模块注册、AppShell、鉴权
    modules/<名字>/  一个功能模块:页面 + api.ts
    shared/          跨模块的组件、hooks、工具、测试基建
  tests/e2e/         Playwright 用例
  src-tauri/         桌面端和安卓端的壳
```

### 加一个前端模块

1. 在 `src/modules/<名字>/` 下写页面和 `api.ts`
2. 在 `src/app/modules.tsx` 里注册路由、导航项、命令面板条目
3. 权限判断写在 `src/modules/auth/permissions.ts` 里,不要散在各个页面

`modules.tsx` 是唯一的注册点——导航栏、路由表、命令面板都从它生成。

## 约定

- **注释解释"为什么",不解释"是什么"。** 代码说得清的事不用重复。约束、权衡、坑,才值得写下来。
- **领域类型挡住无效值。** 新的字符串/数值字段优先在 `domain/` 里建一个带 `parse` 的类型,而不是到处传裸 `String`。
- **权限判定集中在 `access_control.rs`。** handler 里只调用,不自己拼条件。
- **测试跟着行为走。** 加一个接口就在 `tests/api/` 里加对应用例,包括权限被拒的情况。
- 后端注释和文档字符串用英文,面向用户的文案(UI、错误提示、本目录下的文档)用中文。

## 常见问题

**`cargo build` 报 SQL 相关的编译错误**
数据库没起,或者迁移没跑全:

```bash
cd backend
SKIP_DOCKER=true ./scripts/init_db.sh
```

**测试报连不上数据库**
测试用的是 `backend/configuration/base.yaml` 里的超级用户口令(`postgres`/`password`)去建库。确认 Postgres 容器起着,端口是 5432。

**前端报 CORS 错误**
后端的 `cors_allowed_origins` 里要有 `http://127.0.0.1:5173`。`base.yaml` 里默认有,改过就补回去。
