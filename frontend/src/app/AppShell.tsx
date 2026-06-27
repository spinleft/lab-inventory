import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useQueryClient } from "@tanstack/react-query";
import {
  Building2,
  ChevronDown,
  KeyRound,
  LogOut,
  Menu,
  Moon,
  Search,
  Settings,
  SquarePen,
  Sun,
  UserRound,
  X,
} from "lucide-react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useLogout } from "../modules/auth/api";
import { describeRole, describeScope } from "../modules/auth/permissions";
import { federationTrustLabel } from "../modules/federation/api";
import { laboratoryScopeKey, remoteLaboratoryScope } from "../modules/federation/scope";
import { useTheme, type ThemePreference } from "../shared/theme/ThemeProvider";
import { Button } from "../shared/ui/Button";
import { Select } from "../shared/ui/Select";
import { useAuth } from "./auth-context";
import { CommandMenu, useCommandMenuState } from "./CommandMenu";
import {
  LaboratorySelectionProvider,
  useLaboratorySelection,
} from "./laboratory-selection-context";
import { findRoute, moduleNavItems, type ModuleNavItem } from "./modules";

const groupLabels: Record<ModuleNavItem["group"], string> = {
  admin: "管理",
  settings: "设置",
  workspace: "工作区",
};

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { currentUser } = useAuth();
  const logout = useLogout();
  const [commandOpen, setCommandOpen] = useCommandMenuState();
  const visibleNavItems = moduleNavItems.filter(
    (item) => !item.canAccess || item.canAccess(currentUser),
  );
  const currentRoute = findRoute(location.pathname);

  function handleLogout() {
    logout.mutate(undefined, {
      onSettled: () => {
        queryClient.clear();
        navigate("/login", { replace: true });
      },
    });
  }

  return (
    <LaboratorySelectionProvider>
      <div className="app-shell">
        <Sidebar
          items={visibleNavItems}
          isLogoutPending={logout.isPending}
          onCommandOpen={() => setCommandOpen(true)}
          onLogout={handleLogout}
        />
        <div className="app-main">
          <header className="topbar">
            <div className="topbar-left">
              <MobileNavigation
                items={visibleNavItems}
                isLogoutPending={logout.isPending}
                onLogout={handleLogout}
              />
              <span className="breadcrumb">{currentRoute?.title ?? "工作台"}</span>
            </div>
            <div className="topbar-right">
              <ThemeMenu />
            </div>
          </header>
          <div className="page-scroll">
            <Outlet />
          </div>
        </div>
        <CommandMenu open={commandOpen} onOpenChange={setCommandOpen} />
      </div>
    </LaboratorySelectionProvider>
  );
}

function Sidebar({
  items,
  isLogoutPending = false,
  onCommandOpen,
  onLogout,
}: {
  items: ModuleNavItem[];
  isLogoutPending?: boolean;
  onCommandOpen?: () => void;
  onLogout?: () => void;
}) {
  const { currentUser } = useAuth();
  const navigate = useNavigate();

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <Button
              aria-label={`用户菜单 ${currentUser.username}`}
              className="workspace-button user-trigger sidebar-user-trigger"
              variant="ghost"
            >
              <span className="avatar workspace-avatar">
                {currentUser.username.slice(0, 1).toUpperCase()}
              </span>
              <span className="workspace-name">{currentUser.username}</span>
              <ChevronDown className="workspace-caret" size={13} aria-hidden="true" />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content className="dropdown-content" align="start">
              <DropdownMenu.Item
                className="dropdown-item"
                onSelect={() => navigate("/settings/profile")}
              >
                <UserRound size={15} />
                个人资料
              </DropdownMenu.Item>
              <DropdownMenu.Item
                className="dropdown-item"
                onSelect={() => navigate("/settings/password")}
              >
                <KeyRound size={15} />
                修改密码
              </DropdownMenu.Item>
              <DropdownMenu.Item
                className="dropdown-item"
                onSelect={() => navigate("/settings/preferences")}
              >
                <Settings size={15} />
                偏好设置
              </DropdownMenu.Item>
              <DropdownMenu.Separator className="dropdown-separator" />
              <DropdownMenu.Item
                className="dropdown-item"
                disabled={isLogoutPending}
                onSelect={onLogout}
              >
                <LogOut size={15} />
                退出登录
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
        {onCommandOpen ? (
          <div className="sidebar-header-actions">
            <Button
              aria-label="搜索"
              className="sidebar-chrome-button"
              onClick={onCommandOpen}
              size="icon"
              variant="ghost"
            >
              <Search size={14} />
            </Button>
            <Button
              aria-label="新建"
              className="sidebar-chrome-button"
              onClick={onCommandOpen}
              size="icon"
              variant="ghost"
            >
              <SquarePen size={14} />
            </Button>
          </div>
        ) : null}
      </div>
      <nav className="sidebar-scroll" aria-label="主导航">
        {(["workspace", "admin", "settings"] as const).map((group) => {
          const groupItems = items.filter((item) => item.group === group);
          if (groupItems.length === 0) {
            return null;
          }
          return (
            <div className="sidebar-group" key={group}>
              <div className="sidebar-group-label">{groupLabels[group]}</div>
              {groupItems.map((item) => {
                const Icon = item.icon;
                return (
                  <NavLink
                    className={({ isActive }) =>
                      isActive ? "sidebar-link active" : "sidebar-link"
                    }
                    key={item.path}
                    to={item.path}
                  >
                    <Icon size={15} aria-hidden="true" />
                    {item.title}
                  </NavLink>
                );
              })}
            </div>
          );
        })}
      </nav>
      <SidebarLaboratorySelector />
    </aside>
  );
}

function SidebarLaboratorySelector() {
  const {
    canSelectLaboratory,
    federationTrusts,
    federationTrustsLoading,
    laboratories,
    laboratoriesLoading,
    selectedLaboratoryName,
    selectedScopeValue,
    setSelectedScopeValue,
  } = useLaboratorySelection();
  const localOptions = laboratories.map((laboratory) => ({
    label: `本地 · ${laboratory.name}`,
    value: `local:${laboratory.laboratory_id}`,
  }));
  const remoteOptions = federationTrusts.map((trust) => ({
    label: `联邦 · ${federationTrustLabel(trust)}`,
    value: laboratoryScopeKey(
      remoteLaboratoryScope(trust.remote_node_id, trust.remote_laboratory_id),
    ),
  }));
  const scopeOptions = [...localOptions, ...remoteOptions];

  if (!canSelectLaboratory && !selectedLaboratoryName) {
    return null;
  }

  return (
    <div className="sidebar-footer">
      <div className="sidebar-laboratory-label">
        <Building2 size={14} aria-hidden="true" />
        <span>实验室</span>
      </div>
      {canSelectLaboratory ? (
        <Select
          disabled={
            (laboratoriesLoading || federationTrustsLoading) && scopeOptions.length === 0
          }
          label="选择实验室范围"
          options={scopeOptions}
          placeholder="选择实验室"
          value={selectedScopeValue || undefined}
          onValueChange={setSelectedScopeValue}
        />
      ) : (
        <div className="sidebar-laboratory-static" title={selectedLaboratoryName}>
          {selectedLaboratoryName}
        </div>
      )}
    </div>
  );
}

function MobileNavigation({
  items,
  isLogoutPending,
  onLogout,
}: {
  items: ModuleNavItem[];
  isLogoutPending: boolean;
  onLogout: () => void;
}) {
  return (
    <Dialog.Root>
      <Dialog.Trigger asChild>
        <Button
          aria-label="打开导航"
          className="mobile-menu-button"
          size="icon"
          variant="ghost"
        >
          <Menu size={17} />
        </Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content side-panel">
          <div className="dialog-header">
            <Dialog.Title className="dialog-title">导航</Dialog.Title>
            <Dialog.Close asChild>
              <Button size="icon" variant="ghost" aria-label="关闭导航">
                <X size={16} />
              </Button>
            </Dialog.Close>
          </div>
          <div className="dialog-body">
            <Sidebar items={items} isLogoutPending={isLogoutPending} onLogout={onLogout} />
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ThemeMenu() {
  const { preference, setPreference } = useTheme();
  const options: Array<{ label: string; value: ThemePreference }> = [
    { label: "跟随系统", value: "system" },
    { label: "浅色", value: "light" },
    { label: "深色", value: "dark" },
  ];

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button size="icon" variant="ghost" aria-label="切换主题">
          {preference === "dark" ? <Moon size={16} /> : <Sun size={16} />}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="dropdown-content" align="end">
          {options.map((option) => (
            <DropdownMenu.Item
              className="dropdown-item"
              key={option.value}
              onSelect={() => setPreference(option.value)}
            >
              {option.label}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function UserContextLine() {
  const { currentUser } = useAuth();
  return (
    <span>
      {describeRole(currentUser)} · {describeScope(currentUser)}
    </span>
  );
}
