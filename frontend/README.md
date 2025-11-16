# 前端登录页面使用指南

## 📁 项目结构

```
frontend/src/
├── pages/
│   ├── Login.jsx           # 登录页面组件
│   ├── Login.css           # 登录页面样式
│   ├── Dashboard.jsx       # 空白仪表板页面
│   └── Dashboard.css       # 仪表板样式
├── App.jsx                 # 主应用组件（路由配置）
├── App.css                 # 全局应用样式
├── index.css               # 全局基础样式
└── main.jsx                # 应用入口
```

## 🚀 快速开始

### 1. 启动后端服务

确保后端服务器正在运行：

```powershell
cd backend
cargo run
```

后端将在 `http://localhost:8000` 运行。

### 2. 启动前端开发服务器

```powershell
cd frontend
npm run dev
```

前端将在 `http://localhost:5173` 运行。

### 3. 访问登录页面

打开浏览器访问：`http://localhost:5173/login` 或 `http://localhost:5173/`

## 📋 功能说明

### 登录页面 (`/login`)

- **输入字段**：
  - 用户名：必填项
  - 密码：必填项
  
- **功能**：
  - ✅ 表单验证
  - ✅ 加载状态显示
  - ✅ 错误消息显示（登录失败时）
  - ✅ 自动携带 Cookie（`credentials: 'include'`）
  - ✅ 成功后自动跳转到 `/dashboard`

### 仪表板页面 (`/dashboard`)

- 登录成功后的目标页面
- 目前是一个简单的空白欢迎页面
- 可以在此基础上添加更多功能

## 🔧 配置说明

### API 地址配置

在 `Login.jsx` 中配置后端 API 地址：

```javascript
const API_BASE_URL = 'http://localhost:8000';
```

如果后端地址不同，请修改此常量。

### 路由配置

在 `App.jsx` 中配置路由：

```javascript
<Routes>
  <Route path="/login" element={<Login />} />
  <Route path="/dashboard" element={<Dashboard />} />
  <Route path="/" element={<Navigate to="/login" replace />} />
</Routes>
```

- `/` - 自动重定向到 `/login`
- `/login` - 登录页面
- `/dashboard` - 登录成功后的页面

## 🎨 样式说明

### 设计特点

- **渐变背景**：紫色渐变背景（#667eea → #764ba2）
- **卡片设计**：白色圆角卡片，带阴影效果
- **响应式**：自适应不同屏幕尺寸
- **动画效果**：
  - 输入框聚焦效果
  - 按钮悬停效果
  - 错误消息抖动动画

## 🔐 安全特性

### Cookie 管理

前端使用 `credentials: 'include'` 来自动发送和接收 Cookie：

```javascript
fetch(`${API_BASE_URL}/api/v1/auth/login`, {
  method: 'POST',
  credentials: 'include', // 重要！
  // ...
});
```

## 📝 测试流程

### 1. 成功登录测试

1. 访问 `http://localhost:5173/login`
2. 输入正确的用户名和密码
3. 点击"登录"按钮
4. 应该看到：
   - 按钮文字变为"登录中..."
   - 页面跳转到 `/dashboard`
   - 显示欢迎消息

### 2. 失败登录测试

1. 访问 `http://localhost:5173/login`
2. 输入错误的用户名或密码
3. 点击"登录"按钮
4. 应该看到：
   - 红色错误消息框（带抖动动画）
   - 显示错误信息："Authentication failed"
   - 停留在登录页面

## 🐛 常见问题

### 1. CORS 错误

**症状**：控制台显示 CORS 错误

**解决方案**：确保后端 `startup.rs` 中配置了正确的 CORS：

```rust
.allowed_origin("http://localhost:5173")
.supports_credentials()
```

### 2. Cookie 未设置

**症状**：登录后 Cookie 没有被设置

**解决方案**：
- 检查 `credentials: 'include'` 是否正确设置
- 检查后端 `cookie_secure` 设置：
  - 开发环境（HTTP）：设置为 `false`
  - 生产环境（HTTPS）：设置为 `true`

## 📦 依赖包

```json
{
  "dependencies": {
    "react": "^19.1.1",
    "react-dom": "^19.1.1",
    "react-router-dom": "^6.x.x"
  }
}
```

## React Compiler

The React Compiler is currently not compatible with SWC. See [this issue](https://github.com/vitejs/vite-plugin-react/issues/428) for tracking the progress.

## Expanding the ESLint configuration

If you are developing a production application, we recommend using TypeScript with type-aware lint rules enabled. Check out the [TS template](https://github.com/vitejs/vite/tree/main/packages/create-vite/template-react-ts) for information on how to integrate TypeScript and [`typescript-eslint`](https://typescript-eslint.io) in your project.
