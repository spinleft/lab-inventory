## 改了什么

<!-- 简要说明改动内容,以及**为什么**这么改 -->

## 关联 Issue

<!-- 比如 Closes #12 -->

## 自查

- [ ] `cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test` 都过了
- [ ] `npm run typecheck` / `npm run test` / `npm run build` 都过了
- [ ] 改了 `sqlx::query!` 里的 SQL 的话,跑了 `cargo sqlx prepare -- --all-targets` 并提交了 `.sqlx`
- [ ] 加了对应的测试(含权限被拒的情况)
- [ ] 有数据库迁移的话,考虑过已有数据会怎样
- [ ] 影响到部署或配置的话,更新了 `docs/`

## 截图

<!-- 界面改动请附图 -->

## 不兼容变更

<!-- 有的话在这里说明,并写清楚升级时要做什么;没有就删掉这一节 -->
