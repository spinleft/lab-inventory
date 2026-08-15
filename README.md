# Lab Inventory

面向实验室的开源库存管理系统。管理资产台账、库存实物、存放位置和借还流程,支持二维码/条码标签打印、扫码取件,以及跨实验室之间的联邦互查与借用。

*An open-source inventory management system designed for laboratories.*

[![CI](https://github.com/spinleft/lab-inventory/actions/workflows/ci.yml/badge.svg)](https://github.com/spinleft/lab-inventory/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 它解决什么问题

实验室里的东西通常有两种账:一种是"我们有哪些型号的透镜"(资产),另一种是"这批 25mm 的透镜还剩几片、放在哪个抽屉里"(库存实物)。Lab Inventory 把这两层分开建模:

- **资产(Asset)**——一类物品的定义。名称、分类、自定义参数(焦距、波长、量程……)、计量单位。
- **库存实物(Inventory Item)**——实际存在的那些东西。按数量记账(散件、耗材)或按序列号逐个记账(仪器、模块),各自有位置、批次、状态。

在此之上是位置树、计量单位、标签打印、借还审批和审计日志。多实验室场景下,每个实验室的数据互相隔离,由实验室管理员各自维护;需要互通时再通过联邦按需开放。

## 主要功能

| 功能 | 说明 |
| --- | --- |
| 资产与库存 | 分类树、自定义参数(数值/文本/枚举/范围)、数量与序列号两种记账方式、拆分与合并、批量修改 |
| 位置管理 | 树形位置(楼→房间→柜→抽屉),支持按位置检索库存 |
| 标签打印 | 生成二维码/条码标签,直接推送到网络标签打印机(Brother QL 系列) |
| 扫码 | 手机或桌面端扫描标签,直达对应库存条目 |
| 借还 | 提交借用申请、管理员审批、申请人撤回,含完整状态流转 |
| 附件 | 为资产或库存条目上传说明书、照片、校准报告 |
| 多实验室 | 数据按实验室隔离,权限分级到实验室 |
| 联邦 | 两个独立部署的实例配对后,可互相浏览资产、发起跨实验室借用 |
| 访客注册 | 生成注册码,让外部人员自助注册为只读访客 |
| 审计日志 | 记录敏感操作,供系统管理员追溯 |

## 技术构成

- **后端**:Rust + actix-web + sqlx,PostgreSQL 存数据,Redis 存会话。
- **前端**:React 19 + TypeScript + Vite,同一套代码同时用于浏览器、Tauri 桌面端和安卓端。
- **部署**:Docker 镜像 + Docker Compose;也提供独立二进制和静态包。

## 快速开始

先装好 Docker 和 Docker Compose,然后:

```bash
git clone https://github.com/spinleft/lab-inventory.git
cd lab-inventory
cp .env.example .env
# 编辑 .env,至少填上 PUBLIC_URL、HMAC_SECRET、POSTGRES_PASSWORD、INITIAL_ROOT_PASSWORD
docker compose up -d
```

浏览器打开 `.env` 里配置的地址(默认 <http://localhost:8080>),用 `root` 和你设置的 `INITIAL_ROOT_PASSWORD` 登录。

完整步骤、HTTPS 配置、备份和升级见 **[部署文档](docs/deployment.md)**。

## 文档

| 文档 | 内容 |
| --- | --- |
| [快速上手](docs/quickstart.md) | 十分钟跑起来,并录入第一批数据 |
| [部署](docs/deployment.md) | 生产环境部署、反向代理、HTTPS、外部数据库 |
| [配置参考](docs/configuration.md) | 所有配置项及其环境变量写法 |
| [运维](docs/operations.md) | 备份恢复、版本升级、故障排查 |
| [架构](docs/architecture.md) | 代码结构、数据模型、权限模型 |
| [API](docs/api.md) | REST 接口参考 |
| [联邦](docs/federation.md) | 跨实例配对、远程浏览与借用 |
| [标签打印](docs/label-printing.md) | 打印机接入与标签规格 |
| [客户端](docs/clients.md) | 桌面端与安卓端的安装和构建 |
| [开发](docs/development.md) | 本地开发环境、测试、代码约定 |
| [发版](docs/releasing.md) | 维护者发布新版本的流程 |

## 参与贡献

欢迎提 Issue 和 Pull Request,流程见 [CONTRIBUTING.md](CONTRIBUTING.md)。安全问题请按 [SECURITY.md](SECURITY.md) 私下报告,不要开公开 Issue。

## 许可

[MIT](LICENSE)
