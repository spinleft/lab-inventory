# 参与贡献

欢迎提 Issue 和 Pull Request。

## 先说一句

在动手写一个大功能之前,**先开个 Issue 讨论一下**。这能避免你花了几天写完,却发现方向跟项目规划不一致。小的修复(打错字、明显的 bug、文档补充)直接提 PR 就行。

## 报 Bug

开 Issue 时请写清楚:

- 用的什么版本(`docker compose exec backend lab-inventory-admin version`)
- 怎么部署的(Docker Compose / 二进制 / 开发环境)
- 期望是什么,实际是什么
- 复现步骤
- 相关的日志(`docker compose logs backend`)

**日志和截图里的密钥、口令、内部地址记得打码。**

安全漏洞不要开公开 Issue,按 [SECURITY.md](SECURITY.md) 私下报告。

## 开发环境

见 [开发文档](docs/development.md)。简要来说:

```bash
cd backend
./scripts/init_db.sh
./scripts/init_redis.sh
cargo run

cd frontend
npm install
npm run dev
```

## 提 PR

### 提交之前

CI 会卡这几项,本地先跑一遍:

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

改了 `sqlx::query!` 里的 SQL 的话,还要重新生成查询缓存:

```bash
cd backend
cargo sqlx prepare -- --all-targets
git add .sqlx
```

### PR 里写什么

- **改了什么、为什么改。** "为什么"更重要
- 关联的 Issue 编号
- 界面改动配个截图
- 有不兼容变更的话明确说明

### 代码约定

- **注释解释"为什么",不解释"是什么"。** 代码本身说得清的不用重复;约束、权衡、踩过的坑才值得写下来
- **新的字段优先在 `backend/src/domain/` 里建一个带 `parse` 的类型**,而不是到处传裸 `String`
- **权限判断写在 `backend/src/access_control.rs` 里**,不要散在 handler
- **加接口就加测试**,包括权限被拒的情况
- 后端注释和文档字符串用英文,面向用户的文案(界面、错误提示、`docs/` 下的文档)用中文
- 提交信息用中文或英文都行,写清楚做了什么

### 数据库迁移

```bash
cd backend
sqlx migrate add <描述性名字>
```

**已经合进 main 的迁移不要再改内容。** sqlx 会校验每个迁移的校验和,改了之后所有已有部署都会拒绝继续迁移。要改就再加一个。

迁移要考虑**已有数据**:线上跑着的实例执行这个迁移会发生什么?会不会锁表太久?能不能回滚?

## 项目结构

```
backend/     Rust + actix-web,REST API
frontend/    React + TypeScript,浏览器 / 桌面端 / 安卓端共用
docs/        文档
.github/     CI/CD
```

细节见 [架构文档](docs/architecture.md)。

## 行为准则

对事不对人。不欢迎人身攻击、骚扰和歧视性言论。维护者有权删除不当内容。

## 许可

提交代码即表示同意以 [MIT 许可](LICENSE) 发布你的贡献。
