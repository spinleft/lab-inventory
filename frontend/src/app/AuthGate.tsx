import { AlertCircle, Loader2, Server } from "lucide-react";
import { type PropsWithChildren, type ReactNode } from "react";
import { Link, Navigate } from "react-router-dom";
import { useCurrentUser } from "../modules/auth/api";
import { useBackendConfig } from "../shared/api/backendConfig";
import { ApiError } from "../shared/api/httpClient";
import { Button } from "../shared/ui/Button";
import { AuthProvider } from "./auth-context";

export function RequireAuth({ children }: PropsWithChildren) {
  const { hasConfiguredApiBaseUrl } = useBackendConfig();
  const currentUser = useCurrentUser({ enabled: hasConfiguredApiBaseUrl });

  if (!hasConfiguredApiBaseUrl) {
    return <Navigate to="/server-settings" replace />;
  }

  if (currentUser.isLoading) {
    return (
      <EntryStatus
        icon={<Loader2 size={18} />}
        message="正在确认当前会话。"
        title="检查登录状态"
      />
    );
  }

  if (currentUser.error instanceof ApiError && currentUser.error.status === 401) {
    return <Navigate to="/login" replace />;
  }

  if (currentUser.isError || !currentUser.data) {
    return (
      <EntryStatus
        action={
          <Button asChild variant="primary">
            <Link to="/server-settings">
              <Server size={15} />
              服务端设置
            </Link>
          </Button>
        }
        icon={<AlertCircle size={18} />}
        message={currentUser.error?.message ?? "无法连接后端。"}
        title="后端连接异常"
      />
    );
  }

  return <AuthProvider currentUser={currentUser.data}>{children}</AuthProvider>;
}

export function RootRoute() {
  const { hasConfiguredApiBaseUrl } = useBackendConfig();
  const currentUser = useCurrentUser({ enabled: hasConfiguredApiBaseUrl });

  if (!hasConfiguredApiBaseUrl) {
    return <Navigate to="/server-settings" replace />;
  }
  if (currentUser.isLoading) {
    return (
      <EntryStatus
        icon={<Loader2 size={18} />}
        message="正在检查后端会话。"
        title="准备工作台"
      />
    );
  }
  if (currentUser.error instanceof ApiError && currentUser.error.status === 401) {
    return <Navigate to="/login" replace />;
  }
  if (currentUser.isError) {
    return <Navigate to="/server-settings" replace />;
  }
  return <Navigate to="/dashboard" replace />;
}

function EntryStatus({
  action,
  icon,
  message,
  title,
}: {
  action?: ReactNode;
  icon: ReactNode;
  message: string;
  title: string;
}) {
  return (
    <main className="entry-page">
      <section className="entry-card" style={{ width: "min(440px, 100%)" }}>
        <div className="entry-card-inner">
          <div className="entry-brand">
            <span className="brand-mark">LI</span>
            <span>Lab Inventory</span>
          </div>
          <div>
            <h1>{title}</h1>
            <p className="entry-description">
              {icon} {message}
            </p>
          </div>
          {action ? <div className="entry-actions">{action}</div> : null}
        </div>
      </section>
    </main>
  );
}
