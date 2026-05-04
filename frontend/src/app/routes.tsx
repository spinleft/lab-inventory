import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import { AppShell } from "./shell";
import { ProtectedRoute } from "./protectedRoute";
import { LoginPage } from "../features/auth/LoginPage";
import { AssetsPage } from "../features/assets/AssetsPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { InventoryPage } from "../features/inventory/InventoryPage";
import { SettingsPage } from "../features/settings/SettingsPage";

export const routes: RouteObject[] = [
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
      { index: true, element: <DashboardPage /> },
      { path: "assets", element: <AssetsPage /> },
      { path: "inventory", element: <InventoryPage /> },
      { path: "settings", element: <Navigate to="/settings/password" replace /> },
      { path: "settings/password", element: <SettingsPage /> },
    ],
  },
  {
    path: "*",
    element: <Navigate to="/" replace />,
  },
];
