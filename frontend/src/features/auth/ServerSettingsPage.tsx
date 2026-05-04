import { ServerCog } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { ApiError, createApiClient } from "../../shared/api/httpClient";
import { ServerSettingsForm } from "./ServerSettingsForm";

export function ServerSettingsPage() {
  const navigate = useNavigate();

  async function continueAfterSave(apiBaseUrl: string) {
    const client = createApiClient(apiBaseUrl);
    try {
      await client.get("/auth/me");
      navigate("/dashboard", { replace: true });
    } catch (caught) {
      if (caught instanceof ApiError && caught.status === 401) {
        navigate("/login", { replace: true });
        return;
      }
      throw new Error("无法连接后端，请确认地址、网络、CORS 和后端服务状态。");
    }
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
            <h1>后端服务器设置</h1>
            <p>选择当前客户端连接的后端服务。</p>
          </div>
          <div className="server-summary">
            <span>连接目标</span>
            <strong>Lab Inventory API</strong>
          </div>
        </div>

        <div className="login-panel">
          <div className="stack">
            <div className="cluster">
              <ServerCog aria-hidden="true" size={22} />
              <h2>服务器设置</h2>
            </div>
            <p className="muted">
              保存后会根据当前会话状态进入登录页或仪表盘。
            </p>
            <ServerSettingsForm
              actionsClassName="server-settings-actions"
              onSaved={continueAfterSave}
              saveLabel="继续"
              savingLabel="确认中..."
            />
          </div>
        </div>
      </section>
    </main>
  );
}
