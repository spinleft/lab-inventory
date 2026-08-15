# API 参考

REST API 的通用约定和接口清单。所有接口都在 `/api/v1` 下面。

## 约定

### 认证

用 `POST /api/v1/auth/login` 登录,服务端下发一个名叫 `session_id` 的会话 Cookie(HttpOnly、SameSite=Lax、有效期 24 小时)。之后的请求带上这个 Cookie 即可,没有 token、没有 `Authorization` 头。

```bash
# 登录并存下 Cookie
curl -c cookies.txt -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"root","password":"..."}'

# 带着 Cookie 请求
curl -b cookies.txt http://localhost:8080/api/v1/auth/me
```

会话存在 Redis 里。跨域调用要先把来源加进后端的 `cors_allowed_origins`,并且带上 `credentials: "include"`。

### 路径里的作用域

同一批资源有两套路径:

| 前缀 | 谁能用 | 作用域 |
| --- | --- | --- |
| `/api/v1/local/…` | 绑定了实验室的用户(lab_admin、user、guest) | 自己所属的那个实验室,不用在路径里写实验室 id |
| `/api/v1/admin/laboratories/{laboratory_id}/…` | 系统管理员(root、super_admin) | 路径里指定的实验室 |

系统管理员不绑定实验室,所以走 `/local` 会被拒;实验室用户走 `/admin` 也会被拒。

### 分页

列表接口接受 `limit` 和 `offset`:

| 参数 | 默认 | 上限 |
| --- | --- | --- |
| `limit` | 50 | 200(超过按 200 处理,不报错) |
| `offset` | 0 | —— |

响应:

```json
{
  "items": [],
  "limit": 50,
  "offset": 0,
  "total": 137
}
```

### 错误

错误响应统一是:

```json
{ "error": "描述信息" }
```

| 状态码 | 含义 |
| --- | --- |
| 400 | 请求体或参数不合法 |
| 401 | 没登录,或会话过期 |
| 403 | 登录了但没权限做这件事 |
| 404 | 资源不存在,**或者存在但不在你的可见范围内** |
| 409 | 冲突,比如编码重复、状态不允许这个操作 |
| 500 | 服务端错误,详情在服务端日志里 |

403 和 404 的区别值得留意:跨实验室访问别人的资源返回的是 **404 而不是 403**,避免通过错误码探测出别的实验室有哪些资源。

## 接口清单

### 健康检查

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/health_check` | 返回 `{"status":"ok"}`,不需要登录 |

### 认证

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/auth/login` | 登录,下发会话 Cookie |
| POST | `/auth/logout` | 登出 |
| GET | `/auth/me` | 当前用户信息(用户名、角色、所属实验室) |
| PATCH | `/auth/password` | 改自己的密码 |
| POST | `/auth/guest-registration` | 凭注册码自助注册为访客,不需要登录;带频率限制 |

### 实验室

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/local/laboratory` | 自己所属实验室的信息 |
| PATCH | `/local/laboratory` | 改自己实验室的信息 |
| GET | `/admin/laboratories` | 列出所有实验室 |
| POST | `/admin/laboratories` | 新建实验室 |
| GET/PATCH/DELETE | `/admin/laboratories/{id}` | 查/改/删指定实验室 |

### 用户

`/local/users` 和 `/admin/users` 两套,行为一致,区别只是可见范围。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/{scope}/users` | 列出用户 |
| POST | `/{scope}/users` | 新建用户 |
| GET/PATCH/DELETE | `/{scope}/users/{user_id}` | 查/改/删 |
| POST | `/local/guest-registration-codes` | 生成访客注册码 |

谁能管谁,见 [架构文档的权限模型](architecture.md#权限模型)。

### 资产

下面的 `{scope}` 是 `local` 或 `admin/laboratories/{laboratory_id}`。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/{scope}/assets` | 列出资产,支持按分类、参数、关键词过滤 |
| POST | `/{scope}/assets` | 新建资产 |
| GET/PATCH/DELETE | `/{scope}/assets/{asset_id}` | 查/改/删 |
| GET/POST | `/{scope}/assets/{asset_id}/attachments` | 资产附件 |
| POST | `/{scope}/assets/{asset_id}/inventory-items` | 给这个资产入库 |

### 库存实物

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/{scope}/inventory-items` | 列出库存条目 |
| GET/PATCH/DELETE | `/{scope}/inventory-items/{id}` | 查/改/删 |
| PATCH | `/{scope}/inventory-items/batch` | 批量修改 |
| POST | `/{scope}/inventory-items/batch-delete` | 批量删除 |
| POST | `/{scope}/inventory-items/merge` | 合并多条为一条 |
| POST | `/{scope}/inventory-items/{id}/split` | 拆分一条为多条 |
| GET/POST | `/{scope}/inventory-items/{id}/attachments` | 库存条目附件 |

### 基础数据

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET/POST | `/{scope}/asset-categories` | 资产分类树 |
| GET/PATCH/DELETE | `/{scope}/asset-categories/{id}` | |
| GET/POST | `/{scope}/asset-parameters` | 自定义参数定义 |
| GET/PATCH/DELETE | `/{scope}/asset-parameters/{id}` | |
| GET/POST | `/{scope}/locations` | 位置树 |
| GET/PATCH/DELETE | `/{scope}/locations/{id}` | |
| GET/POST | `/{scope}/units` | 计量单位 |
| GET/PATCH/DELETE | `/{scope}/units/{id}` | |

### 附件

附件是两步:先上传文件拿到 upload id,再把它挂到某个资产或库存条目上。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/{scope}/file-uploads` | 上传文件(multipart),返回 upload id |
| DELETE | `/{scope}/file-uploads/{upload_id}` | 丢弃还没挂载的上传 |
| GET | `/{scope}/attachments` | 列出本实验室的附件 |
| GET/PATCH/DELETE | `/{scope}/attachments/{id}` | |
| GET | `/{scope}/attachments/{id}/download` | 下载 |

上传凭证的有效期由 `file_storage.upload_token_ttl_minutes` 决定(默认 60 分钟),过期后没挂载的文件会被清掉。

### 标签打印

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET/POST | `/{scope}/label-printers` | 打印机 |
| GET/PATCH/DELETE | `/{scope}/label-printers/{id}` | |
| GET | `/{scope}/label-printers/{id}/status` | 查打印机状态和已装纸张 |
| POST | `/{scope}/label-printers/{id}/print` | 提交打印任务(客户端渲染好的位图) |
| GET | `/instance-identity` | 本实例的节点 id 和前端地址,生成二维码时用 |

标签的位图是**前端渲染**的,后端只负责转成打印机的光栅指令并送过去。详见 [标签打印文档](label-printing.md)。

### 借用

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/local/inventory-items/{id}/borrow-requests` | 提交借用申请 |
| GET | `/local/borrow-requests` | 待审列表(管理员和普通用户可见) |
| GET | `/local/borrow-requests/mine` | 我提交的申请 |
| PATCH | `/local/borrow-requests/{id}` | 批准或驳回 |
| POST | `/local/borrow-requests/{id}/cancel` | 申请人撤回 |

### 联邦

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/local/federation/pairing-codes` | 生成配对码 |
| POST | `/local/federation/trusts` | 凭对方的配对码建立信任 |
| GET | `/local/federation/trusts` | 已建立的信任 |
| DELETE | `/local/federation/trusts/{id}` | 撤销信任 |
| GET | `/local/federation/guest-links` | 远端访客与本地账号的关联 |
| POST | `/local/federation/guest-links/{id}/merge` | 合并关联 |
| GET/POST | `/federation/nodes/{node_id}/laboratories/{lab_id}/…` | 代理:把请求转发给远端实例 |
| GET/POST | `/federation/inbound/…` | 接收远端实例的请求,签名校验,不走会话 |

`/federation/inbound/*` 不需要登录——它认的是 HMAC 签名。协议细节见 [联邦文档](federation.md)。

### 审计日志

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/audit-logs` | 查审计日志,仅 root 和 super_admin |

## 一个完整的例子

```bash
BASE=http://localhost:8080/api/v1

# 登录
curl -c jar -sX POST $BASE/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"lab-admin","password":"..."}'

# 看看自己是谁
curl -b jar -s $BASE/auth/me

# 新建一个资产
ASSET=$(curl -b jar -sX POST $BASE/local/assets \
  -H 'Content-Type: application/json' \
  -d '{
        "name": "25.4mm 平凸透镜 f=100mm",
        "tracking_mode": "quantity",
        "inventory_unit_id": "<单位 id>"
      }' | jq -r .asset_id)

# 入库
curl -b jar -sX POST $BASE/local/assets/$ASSET/inventory-items \
  -H 'Content-Type: application/json' \
  -d '{"items":[{"quantity_on_hand": 8, "batch_number": "20260816-A"}]}'

# 查库存
curl -b jar -s "$BASE/local/inventory-items?limit=20"
```

> 请求体的具体字段以代码为准:每个资源的字段定义在 `backend/src/routes/<资源>/model.rs`,校验规则在 `backend/src/domain/`。0.1 版本还没有生成式的 OpenAPI 文档。
