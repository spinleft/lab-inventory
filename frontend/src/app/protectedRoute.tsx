import { Navigate } from "react-router-dom";
import { ServerSettingsDialog } from "../features/auth/ServerSettingsDialog";
import { useCurrentUser } from "../features/auth/api";
import { ApiError } from "../shared/api/httpClient";
import { Button } from "../shared/ui/Button";

type ProtectedRouteProps = {
  children: React.ReactNode;
};

export function ProtectedRoute({ children }: ProtectedRouteProps) {
  const currentUser = useCurrentUser();

  if (currentUser.isLoading) {
    return (
      <main className="full-page-center">
        <div className="loading-panel" role="status">
          正在加载会话...
        </div>
      </main>
    );
  }

  if (currentUser.error instanceof ApiError && currentUser.error.status === 401) {
    return <Navigate to="/login" replace />;
  }

  if (currentUser.isError) {
    return (
      <main className="full-page-center">
        <section className="problem-panel">
          <h1>无法连接后端</h1>
          <p>{currentUser.error.message}</p>
          <div className="cluster">
            <ServerSettingsDialog
              trigger={<Button type="button">服务器设置</Button>}
            />
          </div>
        </section>
      </main>
    );
  }

  return children;
}
