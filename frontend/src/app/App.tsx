import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./AppShell";
import { RootRoute } from "./RootRoute";
import { AdminPage } from "../features/admin/AdminPages";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { LoginPage } from "../features/auth/LoginPage";
import { ServerSettingsPage } from "../features/server-settings/ServerSettingsPage";
import {
  PasswordSettingsPage,
  PreferenceSettingsPage,
  ProfileSettingsPage,
} from "../features/settings/SettingsPages";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<RootRoute />} />
      <Route path="/server-settings" element={<ServerSettingsPage />} />
      <Route path="/login" element={<LoginPage />} />
      <Route element={<AppShell />}>
        <Route path="/dashboard" element={<DashboardPage />} />
        <Route path="/settings/user" element={<Navigate to="/settings/profile" replace />} />
        <Route path="/settings/system" element={<Navigate to="/settings/preference" replace />} />
        <Route path="/settings/admin" element={<Navigate to="/admin" replace />} />
        <Route path="/settings/profile" element={<ProfileSettingsPage />} />
        <Route path="/settings/password" element={<PasswordSettingsPage />} />
        <Route path="/settings/preference" element={<PreferenceSettingsPage />} />
        <Route path="/admin" element={<AdminPage />} />
        <Route path="/admin/laboratories" element={<AdminPage section="laboratories" />} />
        <Route path="/admin/remotes" element={<AdminPage section="remotes" />} />
        <Route path="/admin/user" element={<Navigate to="/admin/users" replace />} />
        <Route path="/admin/users" element={<AdminPage section="users" />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

export default App;
