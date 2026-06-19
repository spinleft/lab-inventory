import { CheckCircle, RotateCcw, Server } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  BackendConfigError,
  normalizeApiBaseUrl,
  useBackendConfig,
} from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormField";
import { useToast } from "../../shared/ui/Toast";

export function ServerSettingsPage() {
  const { apiBaseUrl, defaultApiBaseUrl, resetApiBaseUrl, setApiBaseUrl } =
    useBackendConfig();
  const navigate = useNavigate();
  const toast = useToast();
  const [input, setInput] = useState(apiBaseUrl || defaultApiBaseUrl);
  const [error, setError] = useState<string>();
  const [checking, setChecking] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);

    let normalized: string;
    try {
      normalized = normalizeApiBaseUrl(input);
    } catch (caught) {
      setError(
        caught instanceof BackendConfigError ? caught.message : "后端 API 地址无效。",
      );
      return;
    }

    setChecking(true);
    try {
      await checkHealth(normalized);
      setApiBaseUrl(normalized);
      toast.success({ title: "服务端已连接", description: normalized });
      navigate("/login", { replace: true });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "无法连接服务端。");
    } finally {
      setChecking(false);
    }
  }

  function handleReset() {
    const nextDefault = resetApiBaseUrl();
    setInput(nextDefault);
    setError(undefined);
  }

  return (
    <main className="entry-page">
      <div className="entry-shell entry-shell-compact">
        <section className="entry-card entry-card-compact" aria-label="服务端设置">
          <div className="entry-card-inner">
          <div className="entry-brand">
            <span className="brand-mark">LI</span>
            <span>Lab Inventory</span>
          </div>
            <h1 className="entry-compact-title">服务端</h1>
            <form className="entry-form" onSubmit={handleSubmit}>
              <FormField
                error={error}
                hint="示例：http://127.0.0.1:8000/api/v1"
                htmlFor="api-base-url"
                label="后端 API 地址"
              >
                <input
                  className="input"
                  id="api-base-url"
                  value={input}
                  onChange={(event) => setInput(event.target.value)}
                />
              </FormField>
              <div className="entry-actions">
                <Button onClick={handleReset}>
                  <RotateCcw size={15} />
                  重置
                </Button>
                <Button disabled={checking} type="submit" variant="primary">
                  <CheckCircle size={15} />
                  {checking ? "检查中" : "保存并继续"}
                </Button>
              </div>
            </form>
          </div>
        </section>
      </div>
    </main>
  );
}

async function checkHealth(apiBaseUrl: string) {
  const response = await fetch(`${apiBaseUrl.replace(/\/+$/, "")}/health_check`, {
    credentials: "include",
  });
  if (!response.ok) {
    throw new Error(`健康检查失败：HTTP ${response.status}`);
  }
}
