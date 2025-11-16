/**
 * 认证相关 API
 */

const API_BASE_URL = 'http://localhost:8000'; // 后端 API 地址

/**
 * 用户登录
 * @param {string} username - 用户名
 * @param {string} password - 密码
 * @returns {Promise<Object>} 登录响应
 */
export async function login(username, password) {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include', // 重要：允许发送和接收 Cookie
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

/**
 * 获取受保护的资源（示例）
 * @param {string} endpoint - API 端点
 * @returns {Promise<any>} API 响应
 */
export async function fetchProtectedResource(endpoint) {
  const response = await fetch(`${API_BASE_URL}${endpoint}`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include', // 重要：携带会话 Cookie
  });

  if (!response.ok) {
    if (response.status === 401) {
      // 未授权，可能需要重新登录
      throw new Error('Unauthorized');
    }
    throw new Error('Request failed');
  }

  return response.json();
}

/**
 * 用户登出
 * @returns {Promise<void>}
 */
export async function logout() {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/logout`, {
    method: 'POST',
    credentials: 'include',
  });

  if (!response.ok) {
    throw new Error('Logout failed');
  }
}
