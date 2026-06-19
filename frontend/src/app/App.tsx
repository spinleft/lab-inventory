import { Navigate, Route, Routes } from "react-router-dom";
import { LoginPage } from "../modules/auth/LoginPage";
import { ServerSettingsPage } from "../modules/server-settings/ServerSettingsPage";
import { AppShell } from "./AppShell";
import { RequireAuth, RootRoute } from "./AuthGate";
import { moduleRoutes } from "./modules";
import { ProtectedModuleRoute } from "./ProtectedModuleRoute";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<RootRoute />} />
      <Route path="/server-settings" element={<ServerSettingsPage />} />
      <Route path="/login" element={<LoginPage />} />
      <Route
        element={
          <RequireAuth>
            <AppShell />
          </RequireAuth>
        }
      >
        {moduleRoutes.map((route) => (
          <Route
            key={route.id}
            path={route.path}
            element={<ProtectedModuleRoute route={route} />}
          />
        ))}
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
