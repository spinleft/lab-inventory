import { useQueryClient } from "@tanstack/react-query";
import { ArrowRight, Server } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Link, Navigate, useNavigate } from "react-router-dom";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormField";
import { useToast } from "../../shared/ui/Toast";
import { toErrorMessage } from "../../shared/lib/errors";
import { authQueryKeys, useCurrentUser, useLogin } from "./api";

export function LoginPage() {
  const { apiBaseUrl, hasConfiguredApiBaseUrl } = useBackendConfig();
  const currentUser = useCurrentUser({ enabled: hasConfiguredApiBaseUrl });
  const login = useLogin();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  if (!hasConfiguredApiBaseUrl) {
    return <Navigate to="/server-settings" replace />;
  }

  if (currentUser.data) {
    return <Navigate to="/dashboard" replace />;
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    login.mutate(
      { password, username },
      {
        onError: (error) => {
          toast.error({ title: "登录失败", description: toErrorMessage(error) });
        },
        onSuccess: async () => {
          await queryClient.invalidateQueries({ queryKey: authQueryKeys.me(apiBaseUrl) });
          navigate("/dashboard", { replace: true });
        },
      },
    );
  }

  return (
    <main className="entry-page">
      <div className="entry-shell entry-shell-compact">
        <section className="entry-card entry-card-compact" aria-label="登录表单">
          <div className="entry-card-inner">
          <div className="entry-brand">
            <span className="brand-mark">LI</span>
            <span>Lab Inventory</span>
          </div>
            <h1 className="entry-compact-title">登录</h1>
            <form className="entry-form" onSubmit={handleSubmit}>
              <FormField label="用户名" htmlFor="login-username">
                <input
                  autoComplete="username"
                  className="input"
                  id="login-username"
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                />
              </FormField>
              <FormField label="密码" htmlFor="login-password">
                <input
                  autoComplete="current-password"
                  className="input"
                  id="login-password"
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </FormField>
              <div className="entry-actions">
                <Button asChild>
                  <Link to="/server-settings">
                    <Server size={15} />
                    服务端
                  </Link>
                </Button>
                <Button
                  disabled={!username.trim() || !password || login.isPending}
                  type="submit"
                  variant="primary"
                >
                  登录
                  <ArrowRight size={15} />
                </Button>
              </div>
            </form>
          </div>
        </section>
      </div>
    </main>
  );
}
