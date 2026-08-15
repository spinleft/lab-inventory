# Lab Inventory 前端

React + Vite 应用,浏览器版、Tauri 桌面端和安卓端共用这一套代码。

Web UI 与 `src-tauri` 是分开的:Tauri 只是平台外壳,路由、接口调用、主题、交互逻辑和模块注册都由 React 应用自己管。

## 命令

| 命令 | 作用 |
| --- | --- |
| `npm run dev` | 起开发服务器,`127.0.0.1:5173` |
| `npm run typecheck` | 只做类型检查,不产出文件 |
| `npm run test` | Vitest 单元/组件测试 |
| `npm run test:e2e` | Playwright 端到端测试,覆盖桌面/平板/手机三种视口 |
| `npm run build` | 类型检查 + 构建 Web 产物 |
| `npm run tauri:dev` | 在 Tauri 外壳里打开同一个应用 |
| `npm run tauri:build` | 构建桌面端安装包 |
| `npm run tauri:android:build` | 构建安卓 APK(需要先 `tauri:android:init`) |

## 后端地址

默认是 `http://127.0.0.1:8000/api/v1`。实际取值按优先级从高到低:

1. 用户在"服务器设置"页面里设的(存在 localStorage)
2. `config.js` 注入的 `window.__LAB_INVENTORY_CONFIG__.apiBaseUrl`(Docker 镜像在启动时生成)
3. 构建时的 `VITE_DEFAULT_API_BASE_URL`
4. 上面的默认值

详见 [配置参考](../docs/configuration.md#前端配置)。

## 更多

- [开发文档](../docs/development.md) —— 环境搭建、测试、代码约定
- [架构](../docs/architecture.md#前端代码组织) —— 模块注册机制
- [客户端](../docs/clients.md) —— 桌面端和安卓端的构建与分发
