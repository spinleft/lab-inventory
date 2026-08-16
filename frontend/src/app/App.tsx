import { Navigate, Route, Routes } from "react-router-dom";
import { LoginPage } from "../modules/auth/LoginPage";
import { GuestRegisterPage } from "../modules/guests/GuestRegisterPage";
import { ServerSettingsPage } from "../modules/server-settings/ServerSettingsPage";
import { AppShell } from "./AppShell";
import { RequireAuth, RootRoute } from "./AuthGate";
import { moduleRoutes } from "./modules";
import { MorePage } from "./MorePage";
import { ProtectedModuleRoute } from "./ProtectedModuleRoute";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<RootRoute />} />
      <Route path="/server-settings" element={<ServerSettingsPage />} />
      <Route path="/login" element={<LoginPage />} />
      {/* Outside the shell on purpose: whoever opens it has a code, not an
          account. */}
      <Route path="/register" element={<GuestRegisterPage />} />
      <Route
        element={
          <RequireAuth>
            <AppShell />
          </RequireAuth>
        }
      >
        {/* Shell furniture rather than a module: the phone tab bar's overflow. */}
        <Route path="/more" element={<MorePage />} />
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
