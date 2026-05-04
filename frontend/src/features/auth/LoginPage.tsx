import { ServerCog } from "lucide-react";
import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ApiError } from "../../shared/api/httpClient";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { TextInput } from "../../shared/ui/TextInput";
import { ServerSettingsDialog } from "./ServerSettingsDialog";
import { useLogin } from "./api";

export function LoginPage() {
  const navigate = useNavigate();
  const { apiBaseUrl } = useBackendConfig();
  const login = useLogin();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!username.trim() || !password) {
      setValidationError("请输入用户名和密码。");
      return;
    }

    setValidationError(null);
    login.mutate(
      { username: username.trim(), password },
      {
        onSuccess: () => navigate("/", { replace: true }),
      },
    );
  }

  return (
    <main className="full-page-center">
      <section className="login-surface">
        <div className="login-intro">
          <div>
            <div className="brand-block">
              <div className="brand-mark">LI</div>
              <div>
                <div className="brand-title">Lab Inventory</div>
                <div className="brand-subtitle">实验室库存管理</div>
              </div>
            </div>
            <h1>实验室资产与库存工作台</h1>
            <p>
              统一管理设备、材料、库存位置、跨实验室借用和维护提醒。
            </p>
          </div>
          <div className="server-summary">
            <span>当前服务器</span>
            <strong>{apiBaseUrl}</strong>
          </div>
        </div>

        <div className="login-panel">
          <form className="stack" onSubmit={submit}>
            <div>
              <h2>登录</h2>
              <p className="muted">使用后端账户进入库存系统。</p>
            </div>

            <label className="form-row">
              <span className="label">用户名</span>
              <TextInput
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                autoComplete="username"
              />
            </label>
            <label className="form-row">
              <span className="label">密码</span>
              <TextInput
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                autoComplete="current-password"
              />
            </label>

            {validationError ? <div className="alert">{validationError}</div> : null}
            {login.isError ? (
              <div className="alert">{getLoginError(login.error)}</div>
            ) : null}

            <div className="cluster">
              <Button type="submit" disabled={login.isPending}>
                {login.isPending ? "登录中..." : "登录"}
              </Button>
              <ServerSettingsDialog
                trigger={
                  <Button type="button" variant="secondary">
                    <ServerCog size={16} />
                    服务器设置
                  </Button>
                }
              />
            </div>
          </form>
        </div>
      </section>
    </main>
  );
}

function getLoginError(error: Error) {
  if (error instanceof ApiError) {
    return error.message;
  }
  return "登录失败，请检查服务器连接。";
}
