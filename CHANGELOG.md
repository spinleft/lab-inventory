# 更新日志

本文件记录所有值得注意的变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

## [0.1.0] - 2026-08-16

首个可用于生产环境的版本。

### 资产与库存

- 两层库存模型:资产(一类东西的定义)与库存实物(实际存在的东西)分开建模
- 两种记账方式:按数量(支持拆分、合并、批次号)与按序列号(一物一条)
- 资产分类树与位置树,基于 `ltree`,支持子树检索
- 自定义资产参数:数值、文本、枚举、范围四种类型,数值型带单位换算
- 按实验室隔离的计量单位
- 库存条目的批量修改与批量删除
- 附件管理:为资产或库存条目上传文件,按 sha256 去重

### 标签与扫码

- 二维码/条码标签打印,直连 Brother QL 系列网络标签打印机
- 支持 4 种连续纸和 5 种模切标签规格
- 摄像头扫码,扫到即跳转到对应记录
- 二维码格式带节点标识,可以扫开联邦对端实验室打的标签

### 借用

- 库存条目的借用申请、管理员审批、申请人撤回
- 跨实验室的远程借用

### 多实验室与联邦

- 数据按实验室隔离,约束落在数据库层
- 五级角色:root、super_admin、lab_admin、user、guest
- 联邦:两个独立实例配对后可互相浏览资产、发起借用。HMAC 签名 + nonce 防重放 + TLS 证书指纹校验
- 访客注册码:外部人员凭码自助注册为只读访客,带频率限制

### 运维

- 审计日志,记录敏感操作
- `/api/v1/health_check` 健康检查接口

### 部署

- 后端和前端的 Docker 镜像,发布到 GHCR
- 一体化 `docker-compose.yml`,以及使用外部数据库的 `docker-compose.external.yml`
- 生产配置下自动执行数据库迁移,升级只需换镜像 tag
- 前端支持运行时注入后端地址,同一个镜像可用于任意部署
- `lab-inventory-admin` 命令行工具,用于重置密码
- Linux 后端二进制包和前端静态包,供不用 Docker 的部署
- Windows / macOS / Linux 桌面端安装包,以及安卓 APK
- 客户端的 API 请求走原生 HTTP 层,不受 WebView 的同源策略和 SameSite 限制,服务端无需为客户端放行跨域来源

### 界面

- 手机上是一套独立的界面:底部 tab 导航、底部 sheet、列表按卡片排布、筛选收进 sheet,而不是把桌面布局压扁
- 适配刘海屏和手势条,控件按触控尺寸放大
- 桌面端和平板保持原有的侧边栏与表格密度

### 安全

- 迁移脚本里种下的 `root` 出厂密码是公开在源码里的。生产配置下,只要它没被改过,服务就拒绝启动;可以通过 `APP_APPLICATION__INITIAL_ROOT_PASSWORD` 在首次启动时自动替换
- 会话 Cookie 为 HttpOnly + SameSite=Lax,生产配置下要求 HTTPS
- 密码使用 Argon2id
- 跨实验室访问返回 404 而非 403,避免通过错误码探测资源是否存在
- 标签打印机地址禁止回环地址和特权端口,防止被用来探测服务器内网

### 其他

- 前端依赖锁文件重新生成,现在包含所有平台的原生二进制条目。此前的锁文件只记录了 Windows 的条目,`npm ci` 在 Linux 和 macOS 上装不出可用的依赖树([npm/cli#4828](https://github.com/npm/cli/issues/4828))。顺带把直接依赖升到了各自 semver 范围内的最新版
- 后端代码统一 `cargo fmt` 格式,并清掉了全部 Clippy 告警,CI 现在以 `-D warnings` 卡这两项
- 修好了端到端测试:登出菜单的文案、资产接口的查询参数、以及 Playwright 路由注册顺序造成的失配
- 从版本库里移除了不该跟踪的文件:`.env`、前端覆盖率报告、以及 `reference/` 下的参考书 PDF

### 已知限制

- 官方镜像只发布 `linux/amd64`。ARM 机器需要自行构建,见 [部署文档](docs/deployment.md#自己构建镜像)
- 桌面端安装包没有代码签名,Windows 和 macOS 会有安全提示
- 附件只支持存本地磁盘,还没有对象存储后端
- 没有生成式的 OpenAPI 文档
- 客户端不支持离线使用,也没有自动更新

[未发布]: https://github.com/spinleft/lab-inventory/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/spinleft/lab-inventory/releases/tag/v0.1.0
