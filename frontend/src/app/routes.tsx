import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import { AppShell } from "./shell";
import { ProtectedRoute } from "./protectedRoute";
import { AdminPage } from "../features/admin/AdminPage";
import { LoginPage } from "../features/auth/LoginPage";
import { ServerSettingsPage } from "../features/auth/ServerSettingsPage";
import { AssetsPage } from "../features/assets/AssetsPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { InventoryPage } from "../features/inventory/InventoryPage";
import { SettingsPage } from "../features/settings/SettingsPage";

export const routes: RouteObject[] = [
  {
    path: "/",
    element: <ServerSettingsPage />,
  },
  {
    path: "/login",
    element: <LoginPage />,
  },
  {
    path: "/",
    element: (
      <ProtectedRoute>
        <AppShell>
          <Outlet />
        </AppShell>
      </ProtectedRoute>
    ),
    children: [
      { path: "dashboard", element: <DashboardPage /> },
      { path: "assets", element: <AssetsPage /> },
      { path: "inventory", element: <InventoryPage /> },
      { path: "admin", element: <Navigate to="/admin/users" replace /> },
      { path: "admin/users", element: <AdminPage section="users" /> },
      {
        path: "admin/laboratories",
        element: <AdminPage section="laboratories" />,
      },
      { path: "settings", element: <Navigate to="/settings/password" replace /> },
      { path: "settings/password", element: <SettingsPage /> },
    ],
  },
  {
    path: "*",
    element: <Navigate to="/" replace />,
  },
];
