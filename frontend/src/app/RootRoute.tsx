import { Alert, Button, Spin, Space, Typography } from "antd";
import { Link, Navigate } from "react-router-dom";
import { ServerSettingsPage } from "../features/server-settings/ServerSettingsPage";
import { useCurrentUser } from "../features/auth/api";
import { useBackendConfig } from "../shared/api/backendConfig";
import { ApiError } from "../shared/api/httpClient";
import { EntryShell } from "../shared/ui/EntryShell";

const { Text } = Typography;

export function RootRoute() {
  const { hasConfiguredApiBaseUrl } = useBackendConfig();
  const currentUser = useCurrentUser({ enabled: hasConfiguredApiBaseUrl });

  if (!hasConfiguredApiBaseUrl) {
    return <ServerSettingsPage />;
  }

  if (currentUser.isLoading) {
    return (
      <EntryShell
        title="正在检查登录状态"
        titleId="session-check-title"
        description="正在确认当前服务器与本机会话，完成后会自动进入对应页面。"
        cardTitle="状态检查"
      >
        <Space align="center">
          <Spin />
          <Text>正在检查登录状态...</Text>
        </Space>
      </EntryShell>
    );
  }

  if (currentUser.error instanceof ApiError && currentUser.error.status === 401) {
    return <Navigate to="/login" replace />;
  }

  if (currentUser.isError) {
    return (
      <EntryShell
        title="无法连接后端"
        titleId="backend-error-title"
        description="请确认地址、网络、CORS 和后端服务状态。"
        cardTitle="连接异常"
      >
        <Alert showIcon type="error" title={currentUser.error.message} />
        <Link to="/server-settings" className="entry-action-link">
          <Button type="primary" size="large">
            服务器设置
          </Button>
        </Link>
      </EntryShell>
    );
  }

  return <Navigate to="/dashboard" replace />;
}
