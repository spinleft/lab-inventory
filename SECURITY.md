# 安全策略

## 支持的版本

| 版本 | 是否修复安全问题 |
| --- | --- |
| 0.1.x | 是 |

项目还在 0.x 阶段,安全修复只发在最新的次版本上。

## 报告漏洞

**不要开公开 Issue。**

请通过以下任一方式私下报告:

- GitHub 的 [Security Advisory](https://github.com/spinleft/lab-inventory/security/advisories/new)(推荐)
- 邮件:spinleftgit@gmail.com

请尽量包含:

- 漏洞类型和影响
- 复现步骤或概念验证
- 受影响的版本
- 你认为的严重程度

### 我们的响应

- **3 个工作日内**确认收到
- **10 个工作日内**给出初步评估
- 修复后在 Release 说明和 Advisory 中致谢(除非你希望匿名)

这是一个业余维护的开源项目,没有漏洞赏金。

## 部署时请注意

以下几点是自托管者最容易出问题的地方:

### 必须做的

- **改掉 `root` 的出厂密码。** 迁移脚本里种的那个密码公开在本仓库源码中。生产配置(`APP_ENVIRONMENT=production`)下,只要它没被改过,服务就会拒绝启动
- **`HMAC_SECRET` 用随机值。** 至少 64 位:`openssl rand -base64 48`。它同时用于会话 Cookie 加密和访客注册码签名
- **上 HTTPS,并保持 `COOKIE_SECURE=true`。** 会话 Cookie 在 HTTP 上是明文传输的
- **数据库不要暴露到公网。** 默认的 `docker-compose.yml` 不映射 Postgres 端口

### 应该做的

- 日常操作用 `lab_admin` 账号,不要用 `root`
- 定期备份并**演练恢复**,见 [运维文档](docs/operations.md#备份)
- 及时升级:关注 Release 页面
- 开启联邦前读一遍 [联邦文档](docs/federation.md),不要为了图省事关掉 `require_https` 或打开 `allow_insecure_private_network`

## 系统里已有的防护

| 措施 | 说明 |
| --- | --- |
| 密码哈希 | Argon2id(m=15000, t=2, p=1) |
| 会话 | 存在 Redis,Cookie 为 HttpOnly + SameSite=Lax,24 小时有效 |
| SQL 注入 | 全部使用参数化查询,大部分在编译期校验 |
| 跨实验室隔离 | 数据库层的复合外键保证,不只靠应用层判断 |
| 资源探测 | 无权访问的资源返回 404 而非 403 |
| SSRF | 标签打印机地址禁止回环和特权端口;联邦对端默认要求 HTTPS 且拒绝私网地址 |
| 重放攻击 | 联邦请求带时间戳和一次性 nonce |
| 暴力破解 | 访客注册接口有频率限制 |
| 文件上传 | 大小限制,按 sha256 校验和去重 |

## 已知的取舍

这些是当前设计里明确接受的限制,不是漏洞:

- **登录接口没有频率限制。** 需要的话在反向代理层加(Caddy 的 `rate_limit`、nginx 的 `limit_req`)
- **附件下载没有病毒扫描。** 上传的文件原样保存和返回
- **审计日志不覆盖所有操作**,只记录敏感操作
- **`root` 账号无法删除**,只能改密码
