# 登录功能实现说明

## 后端实现（Rust + Actix-web）

### 1. 登录 API (`POST /api/v1/auth/login`)

#### 关键特性：
- ✅ 设置 **HttpOnly** Cookie（防止 XSS 攻击）
- ✅ 设置 **Secure** Cookie（仅通过 HTTPS 传输，生产环境）
- ✅ 设置 **SameSite=Lax** Cookie（防止 CSRF 攻击）
- ✅ 返回 **200 JSON** 响应（而不是重定向）
- ✅ 支持 **CORS** 与 `credentials: 'include'`

#### 请求格式：
```json
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "user123",
  "password": "password123"
}
```

#### 成功响应（200 OK）：
```json
{
  "message": "Login successful"
}
```

#### 失败响应（401 Unauthorized）：
```json
{
  "error": "Authentication failed"
}
```

### 2. Session Cookie 配置

在 `startup.rs` 中的配置：
```rust
SessionMiddleware::builder(redis_store.clone(), secret_key.clone())
    .cookie_name("session_id".to_string())
    .cookie_secure(true)           // 仅 HTTPS（生产环境）
    .cookie_http_only(true)        // 防止 JavaScript 访问
    .cookie_same_site(SameSite::Lax) // CSRF 保护
    .cookie_path("/".to_string())
    .session_lifecycle(
        PersistentSession::default()
            .session_ttl(Duration::hours(24)) // 24小时有效期
    )
    .build()
```

### 3. CORS 配置

允许前端使用 `credentials: 'include'`：
```rust
let cors = Cors::default()
    .allowed_origin("http://localhost:5173")
    .allowed_origin("http://localhost:3000")
    .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
    .allowed_headers(vec![
        actix_web::http::header::AUTHORIZATION,
        actix_web::http::header::ACCEPT,
        actix_web::http::header::CONTENT_TYPE,
    ])
    .supports_credentials() // 关键：支持凭证
    .max_age(3600);
```

## 前端实现（React）

### 1. 登录 API 调用

在 `src/api/auth.js` 中：
```javascript
export async function login(username, password) {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include', // 关键：允许发送和接收 Cookie
    body: JSON.stringify({
      username,
      password,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || 'Login failed');
  }

  return response.json();
}
```

### 2. 登录组件

在 `src/components/Login.jsx` 中：
```javascript
const handleSubmit = async (e) => {
  e.preventDefault();
  
  try {
    const response = await login(username, password);
    console.log('Login successful:', response);
    
    // 登录成功后跳转到受保护页面
    // Cookie 已自动设置（HttpOnly + Secure + SameSite）
    navigate('/dashboard');
  } catch (err) {
    setError(err.message || 'Login failed');
  }
};
```

### 3. 访问受保护资源

所有需要认证的请求都必须包含 `credentials: 'include'`：
```javascript
export async function fetchProtectedResource(endpoint) {
  const response = await fetch(`${API_BASE_URL}${endpoint}`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include', // 携带会话 Cookie
  });

  if (!response.ok) {
    if (response.status === 401) {
      throw new Error('Unauthorized');
    }
    throw new Error('Request failed');
  }

  return response.json();
}
```

## 安全性说明

### 1. Cookie 属性
- **HttpOnly**: Cookie 无法被 JavaScript 访问，防止 XSS 攻击窃取会话
- **Secure**: Cookie 仅通过 HTTPS 传输（开发环境可设为 false）
- **SameSite=Lax**: 防止 CSRF 攻击，仅在同站点请求中发送

### 2. CORS 配置
- **allowed_origin**: 只允许特定前端域名访问
- **supports_credentials**: 必须设置为 true 才能接收带凭证的请求
- **前端 credentials: 'include'**: 必须在每个请求中设置

### 3. 会话管理
- 使用 Redis 存储会话数据（可扩展）
- 登录成功后调用 `session.renew()` 防止会话固定攻击
- 会话有效期为 24 小时

## 开发环境设置

### 1. 后端（开发环境）

如果在本地开发中使用 HTTP（非 HTTPS），需要将 `cookie_secure` 设置为 `false`：

```rust
.cookie_secure(false) // 开发环境使用 HTTP
```

**生产环境务必设置为 `true`！**

### 2. 前端

确保 `vite.config.js` 配置正确的代理（可选）：
```javascript
export default {
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
      },
    },
  },
}
```

## 依赖项

### 后端 Cargo.toml
```toml
actix-cors = "0.7"
actix-session = { version = "0.11", features = ["redis-session-rustls"] }
actix-web = "4"
```

### 前端 package.json
```json
{
  "dependencies": {
    "react": "^18.2.0",
    "react-router-dom": "^6.0.0"
  }
}
```

## 测试流程

1. 启动 Redis：
   ```bash
   docker run -d -p 6379:6379 redis:alpine
   ```

2. 启动后端：
   ```bash
   cd backend
   cargo run
   ```

3. 启动前端：
   ```bash
   cd frontend
   npm run dev
   ```

4. 访问 http://localhost:5173 并测试登录

## 注意事项

⚠️ **生产环境检查清单**：
- [ ] 将 `cookie_secure(true)` 启用（仅 HTTPS）
- [ ] 配置正确的 CORS allowed_origin（不要使用 `*`）
- [ ] 使用强密钥生成 `hmac_secret`
- [ ] 配置 Redis 的安全访问
- [ ] 启用 HTTPS/TLS
- [ ] 设置合适的会话过期时间
- [ ] 实现登出功能清除会话

## 下一步

实现以下功能以完善认证系统：
1. 登出 API (`POST /api/v1/auth/logout`)
2. 会话验证中间件（保护其他 API）
3. 刷新令牌机制（可选）
4. 记住我功能（可选）
