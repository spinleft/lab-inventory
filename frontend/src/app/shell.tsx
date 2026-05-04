import {
  Boxes,
  LayoutDashboard,
  LogOut,
  PackageSearch,
  ServerCog,
  Settings,
  ShieldCheck,
} from "lucide-react";
import { NavLink, useNavigate } from "react-router-dom";
import { ServerSettingsDialog } from "../features/auth/ServerSettingsDialog";
import { useCurrentUser, useLogout } from "../features/auth/api";
import { canAccessAdminArea } from "../features/auth/permissions";
import { IconButton } from "../shared/ui/IconButton";

type AppShellProps = {
  children: React.ReactNode;
};

type NavItem = {
  to: string;
  label: string;
  icon: typeof LayoutDashboard;
  end?: boolean;
};

const baseNavItems: NavItem[] = [
  { to: "/dashboard", label: "概览", icon: LayoutDashboard },
  { to: "/assets", label: "资产", icon: PackageSearch },
  { to: "/inventory", label: "库存", icon: Boxes },
];

export function AppShell({ children }: AppShellProps) {
  const navigate = useNavigate();
  const currentUser = useCurrentUser();
  const logout = useLogout();
  const user = currentUser.data;
  const navItems = canAccessAdminArea(user)
    ? [
        ...baseNavItems,
        { to: "/admin/users", label: "管理区", icon: ShieldCheck },
      ]
    : baseNavItems;

  return (
    <div className="app-layout">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand-block">
          <div className="brand-mark">LI</div>
          <div>
            <div className="brand-title">Lab Inventory</div>
            <div className="brand-subtitle">实验室库存</div>
          </div>
        </div>
        <nav className="nav-list">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  isActive ? "nav-link nav-link-active" : "nav-link"
                }
              >
                <Icon aria-hidden="true" size={18} />
                <span>{item.label}</span>
              </NavLink>
            );
          })}
        </nav>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <div>
            <div className="topbar-user">{user?.username ?? "未知用户"}</div>
            <div className="topbar-meta">
              {user?.laboratory?.name ?? "系统所有者"} ·{" "}
              {user?.user_type.name ?? "unknown"}
            </div>
          </div>
          <div className="toolbar">
            <IconButton label="用户设置" onClick={() => navigate("/settings/password")}>
              <Settings size={18} />
            </IconButton>
            <ServerSettingsDialog
              trigger={
                <IconButton label="服务器设置">
                  <ServerCog size={18} />
                </IconButton>
              }
            />
            <IconButton
              label="退出登录"
              onClick={() =>
                logout.mutate(undefined, {
                  onSettled: () => navigate("/login", { replace: true }),
                })
              }
            >
              <LogOut size={18} />
            </IconButton>
          </div>
        </header>
        <main className="content">{children}</main>
      </div>
    </div>
  );
}
