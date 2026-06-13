import {
  ApartmentOutlined,
  ApiOutlined,
  AuditOutlined,
  DashboardOutlined,
  DatabaseOutlined,
  LockOutlined,
  LogoutOutlined,
  ProfileOutlined,
  SettingOutlined,
  SwapOutlined,
  ToolOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Space, Spin, Typography, type MenuProps } from "antd";
import { createContext, useContext, type PropsWithChildren } from "react";
import { Link, Navigate, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useCurrentUser, useLogout } from "../features/auth/api";
import {
  canAccessAdminSettings,
  describeRole,
  describeScope,
} from "../features/auth/permissions";
import { type CurrentUser } from "../features/auth/types";
import { useBackendConfig } from "../shared/api/backendConfig";
import { ApiError } from "../shared/api/httpClient";
import { AppChrome, type AppChromeNavItem } from "../shared/ui/AppChrome";
import { EntryShell } from "../shared/ui/EntryShell";

const { Text } = Typography;

type AppShellContextValue = {
  currentUser: CurrentUser;
};

const AppShellContext = createContext<AppShellContextValue | null>(null);

const pageTitles: Record<string, string> = {
  "/dashboard": "概览",
  "/settings/profile": "用户资料",
  "/settings/password": "密码",
  "/settings/preference": "偏好设置",
  "/admin": "管理中心",
  "/admin/laboratories": "实验室",
  "/admin/remotes": "远端实验室",
  "/admin/users": "用户",
};

export function AppShell() {
  const { hasConfiguredApiBaseUrl } = useBackendConfig();
  const currentUser = useCurrentUser({ enabled: hasConfiguredApiBaseUrl });

  if (!hasConfiguredApiBaseUrl) {
    return <Navigate to="/server-settings" replace />;
  }

  if (currentUser.isLoading) {
    return (
      <EntryShell
        title="正在检查登录状态"
        titleId="app-shell-session-check-title"
        description="正在确认当前服务器与本机会话，完成后会自动进入后台。"
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

  if (currentUser.isError || !currentUser.data) {
    return (
      <EntryShell
        title="无法连接后端"
        titleId="app-shell-backend-error-title"
        description="请确认地址、网络、CORS 和后端服务状态。"
        cardTitle="连接异常"
      >
        <Alert
          showIcon
          type="error"
          title={currentUser.error?.message ?? "请求失败。"}
        />
        <Link to="/server-settings" className="entry-action-link">
          <Button type="primary" size="large">
            服务器设置
          </Button>
        </Link>
      </EntryShell>
    );
  }

  return (
    <AppShellContext.Provider value={{ currentUser: currentUser.data }}>
      <AuthenticatedShell currentUser={currentUser.data}>
        <Outlet />
      </AuthenticatedShell>
    </AppShellContext.Provider>
  );
}

function AuthenticatedShell({
  children,
  currentUser,
}: PropsWithChildren<{ currentUser: CurrentUser }>) {
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const logout = useLogout();
  const currentPath = normalizePath(location.pathname);
  const navigationDomain = getNavigationDomain(currentPath);
  const title = pageTitles[currentPath] ?? "后台";
  const sidebarItems = getSidebarItems(navigationDomain, currentUser);
  const sidebarTitle = getSidebarTitle(navigationDomain);
  const selectedSidebarKey = sidebarItems.some((item) => item.key === currentPath)
    ? currentPath
    : undefined;

  const userMenuItems: MenuProps["items"] = [
    {
      key: "settings-user",
      icon: <UserOutlined />,
      label: "用户设置",
    },
    ...(canAccessAdminSettings(currentUser)
      ? [
          {
            key: "settings-admin",
            icon: <ApartmentOutlined />,
            label: "管理中心",
          },
        ]
      : []),
    {
      type: "divider" as const,
    },
    {
      key: "logout",
      danger: true,
      icon: <LogoutOutlined />,
      label: "登出",
    },
  ];

  function handleUserMenuClick({ key }: Parameters<NonNullable<MenuProps["onClick"]>>[0]) {
    if (key === "settings-user") {
      navigate("/settings/profile");
      return;
    }
    if (key === "settings-admin") {
      navigate("/admin");
      return;
    }
    if (key === "logout") {
      logout.mutate(undefined, {
        onSettled: () => {
          queryClient.clear();
          navigate("/login", { replace: true });
        },
      });
    }
  }

  return (
    <AppChrome
      breadcrumbItems={[
        {
          key: "dashboard",
          label: "后台",
          onClick: () => navigate("/dashboard"),
        },
        { key: currentPath, label: title },
      ]}
      isUserMenuLoading={logout.isPending}
      onBrandClick={() => navigate("/dashboard")}
      onSidebarSelect={(key) => navigate(key)}
      onUserMenuClick={handleUserMenuClick}
      pageIcon={<ToolOutlined aria-hidden="true" className="app-page-title-icon" />}
      pageMeta={
        <Text type="secondary">
          {describeRole(currentUser)} · {describeScope(currentUser)}
        </Text>
      }
      pageTitle={title}
      selectedSidebarKey={selectedSidebarKey}
      sidebarItems={sidebarItems}
      sidebarTitle={sidebarTitle}
      userInitial={currentUser.username.slice(0, 1).toUpperCase()}
      userMenuItems={userMenuItems}
      userName={currentUser.username}
    >
      {children}
    </AppChrome>
  );
}

export function useAppShell() {
  const context = useContext(AppShellContext);
  if (!context) {
    throw new Error("useAppShell must be used inside AppShell.");
  }
  return context;
}

function normalizePath(pathname: string) {
  if (pathname.startsWith("/settings/user")) {
    return "/settings/profile";
  }
  if (pathname.startsWith("/settings/profile")) {
    return "/settings/profile";
  }
  if (pathname.startsWith("/settings/password")) {
    return "/settings/password";
  }
  if (pathname.startsWith("/settings/system")) {
    return "/settings/preference";
  }
  if (pathname.startsWith("/settings/preference")) {
    return "/settings/preference";
  }
  if (pathname.startsWith("/settings/admin")) {
    return "/admin";
  }
  if (pathname.startsWith("/admin/laboratories")) {
    return "/admin/laboratories";
  }
  if (pathname.startsWith("/admin/remotes")) {
    return "/admin/remotes";
  }
  if (pathname.startsWith("/admin/users")) {
    return "/admin/users";
  }
  if (pathname.startsWith("/admin")) {
    return "/admin";
  }
  return "/dashboard";
}

type NavigationDomain = "dashboard" | "settings" | "admin";

function getNavigationDomain(pathname: string): NavigationDomain {
  if (pathname.startsWith("/settings/")) {
    return "settings";
  }
  if (pathname.startsWith("/admin")) {
    return "admin";
  }
  return "dashboard";
}

function getSidebarTitle(navigationDomain: NavigationDomain) {
  if (navigationDomain === "settings") {
    return "设置导航";
  }
  if (navigationDomain === "admin") {
    return "管理导航";
  }
  return "后台导航";
}

function getSidebarItems(
  navigationDomain: NavigationDomain,
  currentUser: CurrentUser,
): AppChromeNavItem[] {
  if (navigationDomain === "settings") {
    return [
      {
        key: "/settings/profile",
        icon: <ProfileOutlined />,
        label: "用户资料",
      },
      {
        key: "/settings/password",
        icon: <LockOutlined />,
        label: "密码",
      },
      {
        key: "/settings/preference",
        icon: <SettingOutlined />,
        label: "偏好设置",
      },
    ];
  }

  if (navigationDomain === "admin") {
    if (!canAccessAdminSettings(currentUser)) {
      return [];
    }
    return [
      {
        key: "/admin/laboratories",
        icon: <ApartmentOutlined />,
        label: "实验室",
      },
      {
        key: "/admin/users",
        icon: <UserOutlined />,
        label: "用户",
      },
      {
        key: "/admin/remotes",
        icon: <ApiOutlined />,
        label: "远端实验室",
      },
    ];
  }

  return [
    {
      key: "/dashboard",
      icon: <DashboardOutlined />,
      label: "概览",
    },
    {
      key: "/inventory",
      icon: <DatabaseOutlined />,
      label: "库存",
      disabled: true,
    },
    {
      key: "/borrow-requests",
      icon: <SwapOutlined />,
      label: "借用",
      disabled: true,
    },
    {
      key: "/maintenance",
      icon: <ToolOutlined />,
      label: "维护",
      disabled: true,
    },
    {
      key: "/audit-logs",
      icon: <AuditOutlined />,
      label: "审计日志",
      disabled: true,
    },
  ];
}
