# 架构

代码结构、数据模型和权限模型。想动代码的话,配合 [开发文档](development.md) 看。

## 总体

```
┌─────────────────────────────────────────────┐
│  前端 (React 19 + TypeScript + Vite)        │
│  浏览器 / Tauri 桌面端 / Tauri 安卓端        │
│  同一套代码,只有外壳不同                    │
└──────────────────┬──────────────────────────┘
                   │ REST + 会话 Cookie
┌──────────────────▼──────────────────────────┐
│  后端 (Rust + actix-web)                     │
│                                              │
│  routes/         HTTP 层                     │
│  access_control  权限判定                    │
│  domain/         领域类型与校验              │
│  file_storage/   附件                        │
│  label_printing/ 打印机协议                  │
└─────┬────────────────────────┬───────────────┘
      │ sqlx                   │
┌─────▼──────┐        ┌────────▼────────┐
│ PostgreSQL │        │      Redis      │
│ 业务数据    │        │      会话       │
└────────────┘        └─────────────────┘
```

## 数据模型

### 两层库存

这是整个系统最核心的建模决定:

```
Asset(资产)                  一类东西的定义
  ├─ 名称、分类、计量单位
  ├─ 自定义参数值(焦距、波长……)
  └─ tracking_mode: quantity | serialized
        │
        └── InventoryItem(库存实物)  实际存在的东西
              ├─ 位置、状态
              ├─ quantity 模式:数量 + 批次号
              └─ serialized 模式:序列号,一物一条
```

`tracking_mode` 在资产上定死,库存条目必须跟它一致——这一条是靠把 `tracking_mode` 放进外键里保证的,应用层写错也插不进去。两种模式的操作语义完全不同:数量模式支持拆分/合并,序列号模式则要求每条记录唯一对应一个实物。

### 实验室隔离

几乎每张业务表都有 `laboratory_id`,并且有跨表一致性约束——比如库存条目的 `laboratory_id` 必须跟它所属资产的一致,位置也必须属于同一个实验室。这些约束落在数据库上,而不只是在应用层判断:

```sql
-- 库存条目上的位置不可能是别的实验室的位置:外键带上了 laboratory_id
FOREIGN KEY (location_id, laboratory_id)
    REFERENCES locations (location_id, laboratory_id),
-- 同理,它的资产必须同实验室,而且记账方式必须一致
FOREIGN KEY (asset_id, laboratory_id, tracking_mode)
    REFERENCES assets (asset_id, laboratory_id, tracking_mode)
    ON DELETE CASCADE
```

计量单位、资产分类、位置也都是按实验室隔离的。新建的实验室是一张白纸,要自己建这些基础数据。

### 树形结构

位置和资产分类都用 PostgreSQL 的 `ltree` 存路径,配合 GiST 索引,查子树时不用递归:

```sql
SELECT * FROM locations WHERE path <@ 'building4.room113';
```

### 自定义参数

```
AssetParameterType(参数定义)      焦距、波长、量程……
  ├─ data_type: number | text | enum | range
  ├─ unit(数值型)
  └─ options(枚举型)
        │
        ├── 绑定到 AssetCategory ── 这个分类下的资产才有这个参数
        └── AssetParameterValue ── 具体某个资产的取值
```

数值型参数带单位,检索时会做单位换算(查"焦距 > 10cm"能匹配到填了 100mm 的资产)。

### 附件

附件是两步走的,为的是让上传和挂载解耦:

```
FileUpload(临时)  ──挂载──▶  File + AttachmentAssignment(正式)
   有效期 60 分钟                  挂到某个资产或某个库存条目
```

没挂载的上传会过期清理。文件按 sha256 去重,同一个文件传两次只占一份空间。

### 联邦

```
federation_local_nodes         本实例的身份(单行表,node_id 永不改变)
federation_remote_nodes        已配对的对端 + 共享密钥 + TLS 证书指纹
federation_laboratory_trusts   实验室级授权:本地实验室 → 远端实验室
federation_request_nonces      防重放
federation_guest_links         远端用户在本地的影子账号
federation_borrow_requests     远程借用申请
```

详见 [联邦文档](federation.md)。

## 权限模型

### 角色

| 角色 | 绑定实验室 | 范围 |
| --- | --- | --- |
| `root` | 否 | 一切。唯一能管理其他系统管理员的角色 |
| `super_admin` | 否 | 跨实验室的一切,除了管理 root |
| `lab_admin` | 是 | 本实验室的一切:用户、资产、库存、打印机、联邦 |
| `user` | 是 | 本实验室的日常操作:增删改资产和库存、打标签、借用 |
| `guest` | 是 | 本实验室只读 + 提交借用申请 |

### 两条路径

权限的第一道关是路径本身:

- `/api/v1/local/*` —— `reject_non_laboratory_users` 中间件,只放行绑定了实验室的用户
- `/api/v1/admin/*` —— `reject_non_system_admins` 中间件,只放行 root 和 super_admin

系统管理员走 `/local` 会被拒(他们不属于任何实验室),实验室用户走 `/admin` 也会被拒。这样"你能操作哪个实验室"在路由层就定下来了,handler 里不用反复判断。

### 判定集中在一处

所有细粒度判断都在 `backend/src/access_control.rs` 里,handler 只调用不自己拼:

```rust
if !actor.can_write_laboratory_resource(laboratory_id) {
    return Err(Error::Forbidden(...));
}
```

前端有一份对应的 `frontend/src/modules/auth/permissions.ts`,用来决定按钮显不显示。**前端那份只影响界面,不是安全边界**——所有判断在后端都会重做一遍。

### 看不见的东西返回 404

跨实验室访问别人的资源返回 **404 而不是 403**。403 等于承认"这个 id 存在但你没权限",能被用来探测别的实验室有哪些资源。

### 谁能管谁

| 操作者 | 能管理 |
| --- | --- |
| root | 所有人 |
| super_admin | 除 root 外的所有人 |
| lab_admin | 本实验室内的 lab_admin、user、guest |
| 任何人 | 自己(改自己的密码和资料) |

## 后端代码组织

```
backend/src/
  domain/           领域类型。一个文件一个类型,带 parse 校验
  routes/<资源>/
    create.rs get.rs list.rs update.rs delete.rs   handler,一个动作一个文件
    model.rs        请求/响应结构
    queries.rs      SQL
    service.rs      编排逻辑
  authentication/   登录、密码、中间件、访客注册
  access_control.rs 权限判定
  file_storage/     附件存储抽象(目前只有本地实现)
  label_printing/   光栅编码、状态解析、TCP 传输
  idempotency/      幂等键
  audit.rs          审计日志
  bootstrap.rs      启动时的迁移和 root 密码保护
  configuration.rs  配置结构
  startup.rs        路由注册和中间件装配
  telemetry.rs      结构化日志
```

### 领域类型

无效值在类型系统边界外就被挡住,而不是靠 handler 里散落的 if:

```rust
pub struct UnitCode(String);

impl UnitCode {
    pub fn parse(s: String) -> Result<UnitCode, String> {
        // 长度、字符集、非空……
    }
}
```

handler 拿到的一定是已经校验过的值,后面的代码不用再防御。

### 错误处理

每个 handler 有自己的错误枚举,实现 `ResponseError` 把变体映射到状态码:

```rust
impl ResponseError for CreateAssetError {
    fn status_code(&self) -> StatusCode {
        match self {
            ValidationError(_) => StatusCode::BAD_REQUEST,
            Forbidden(_)       => StatusCode::FORBIDDEN,
            ConflictError(_)   => StatusCode::CONFLICT,
            UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
```

内部错误的细节只进日志,不进响应体。

## 前端代码组织

```
frontend/src/
  app/
    modules.tsx      模块注册表 —— 路由、导航、命令面板都从这里生成
    AppShell.tsx     整体布局
    AuthGate.tsx     登录态门禁
    providers.tsx    全局 Provider
  modules/<名字>/
    XxxPage.tsx      页面
    api.ts           这个模块的接口调用 + zod schema
  shared/
    api/             HTTP 客户端、后端地址配置
    components/      跨模块组件
    lib/             工具函数
    test/            测试基建(MSW handlers、render 包装)
```

### 模块注册

`modules.tsx` 是唯一的注册点。加一个功能模块 = 写页面 + 在这里登记一条。导航栏、路由表、命令面板(Ctrl+K)都从这张表生成,不会出现"路由加了但导航里没有"的情况。

每条登记项可以带 `canAccess`,决定当前用户看不看得到。

### 后端地址

前端不假设后端在哪里。地址按优先级取:用户在界面里设的 → `config.js` 运行时注入的 → 构建时烘进去的 → 本地开发默认值。这样同一份构建产物既能做成 Docker 镜像同源部署,也能打包成桌面端连任意服务器。

## 状态与会话

- 会话存在 Redis,Cookie 是 HttpOnly + SameSite=Lax,24 小时有效
- 密码用 Argon2id(m=15000, t=2, p=1)
- 前端用 TanStack Query 管服务端状态,不自己维护缓存

## 测试

- **后端集成测试**(`backend/tests/api/`)是主力。每个用例起一个真实的 app 实例、建一个独立的数据库,跑完删掉。覆盖正常路径、权限被拒、跨实验室隔离
- **后端单元测试**跟在被测模块里,覆盖领域类型的校验和协议编码这类纯逻辑
- **前端组件测试**(Vitest + Testing Library + MSW)
- **端到端测试**(Playwright)覆盖桌面、平板、手机三种视口,自己拦截 API 请求,不依赖后端
