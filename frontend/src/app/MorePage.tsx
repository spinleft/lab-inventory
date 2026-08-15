import { ChevronRight, LogOut, Moon, Smartphone, Sun } from "lucide-react";
import { Link } from "react-router-dom";
import { describeRole, describeScope } from "../modules/auth/permissions";
import { useTheme, type ThemePreference } from "../shared/theme/ThemeProvider";
import { Button } from "../shared/ui/Button";
import { useAuth } from "./auth-context";
import { mobileTabItems } from "./MobileShell";
import { moduleNavItems, type ModuleNavItem } from "./modules";
import { useLogoutAction } from "./useLogoutAction";

const groupLabels: Record<ModuleNavItem["group"], string> = {
  admin: "管理",
  settings: "设置",
  workspace: "工作区",
};

const themeOptions: Array<{ icon: typeof Sun; label: string; value: ThemePreference }> = [
  { icon: Smartphone, label: "跟随系统", value: "system" },
  { icon: Sun, label: "浅色", value: "light" },
  { icon: Moon, label: "深色", value: "dark" },
];

/**
 * The phone shell's fifth tab: everything the four other tabs cannot hold.
 *
 * Only reachable below the mobile breakpoint — on desktop the same entries are
 * in the sidebar, and the router keeps the route so a rotated tablet or a
 * resized window never lands on a dead URL.
 */
export function MorePage() {
  const { currentUser } = useAuth();
  const { preference, setPreference } = useTheme();
  const { isPending, logout } = useLogoutAction();

  const visible = moduleNavItems.filter(
    (item) => !item.canAccess || item.canAccess(currentUser),
  );
  const inTabs = new Set(mobileTabItems(visible).map((item) => item.path));
  const rest = visible.filter((item) => !inTabs.has(item.path));

  return (
    <div className="page more-page">
      <section className="more-identity">
        <span className="avatar more-avatar">
          {currentUser.username.slice(0, 1).toUpperCase()}
        </span>
        <div className="more-identity-text">
          <strong>{currentUser.username}</strong>
          <span className="more-identity-meta">
            {describeRole(currentUser)} · {describeScope(currentUser)}
          </span>
        </div>
      </section>

      {(["workspace", "admin", "settings"] as const).map((group) => {
        const groupItems = rest.filter((item) => item.group === group);
        if (groupItems.length === 0) {
          return null;
        }
        return (
          <section className="more-group" key={group}>
            <h2 className="more-group-label">{groupLabels[group]}</h2>
            <ul className="more-list">
              {groupItems.map((item) => (
                <li key={item.path}>
                  <Link className="more-row" to={item.path}>
                    <item.icon size={18} aria-hidden="true" />
                    <span className="more-row-label">{item.title}</span>
                    <ChevronRight className="more-row-chevron" size={17} aria-hidden="true" />
                  </Link>
                </li>
              ))}
            </ul>
          </section>
        );
      })}

      <section className="more-group">
        <h2 className="more-group-label">外观</h2>
        <div className="more-segmented" role="group" aria-label="主题">
          {themeOptions.map((option) => (
            <button
              aria-pressed={preference === option.value}
              className={
                preference === option.value ? "more-segment active" : "more-segment"
              }
              key={option.value}
              type="button"
              onClick={() => setPreference(option.value)}
            >
              <option.icon size={16} aria-hidden="true" />
              {option.label}
            </button>
          ))}
        </div>
      </section>

      <Button
        className="more-logout"
        disabled={isPending}
        variant="ghost"
        onClick={logout}
      >
        <LogOut size={16} />
        退出登录
      </Button>
    </div>
  );
}
