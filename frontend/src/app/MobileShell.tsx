import * as Dialog from "@radix-ui/react-dialog";
import { Building2, ChevronLeft, Check, Ellipsis } from "lucide-react";
import { type ReactNode } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { federationTrustLabel } from "../modules/federation/api";
import { laboratoryScopeKey, remoteLaboratoryScope } from "../modules/federation/scope";
import { Button } from "../shared/ui/Button";
import { useLaboratorySelection } from "./laboratory-selection-context";
import { findRoute, type ModuleNavItem } from "./modules";

/**
 * The paths that get a bottom tab, most wanted first.
 *
 * Roles see different subsets — a guest has no review queue, an admin has no
 * "我的借用" — so this is a preference order rather than a fixed bar: whatever
 * the user can actually reach fills the four slots left by "更多".
 */
const TAB_PREFERENCE = [
  "/dashboard",
  "/inventory",
  "/scan",
  "/borrow-requests",
  "/borrow-requests/mine",
  "/assets",
];

const MORE_PATH = "/more";

export function mobileTabItems(items: ModuleNavItem[]) {
  const byPath = new Map(items.map((item) => [item.path, item]));
  const preferred = TAB_PREFERENCE.map((path) => byPath.get(path)).filter(
    (item): item is ModuleNavItem => Boolean(item),
  );
  // Anything the preference list does not name still deserves a slot if there
  // is room, so a stripped-down role does not end up with two tabs.
  const rest = items.filter(
    (item) => item.group === "workspace" && !TAB_PREFERENCE.includes(item.path),
  );
  return [...preferred, ...rest].slice(0, 4);
}

export function MobileShell({ items }: { items: ModuleNavItem[] }) {
  const location = useLocation();
  const navigate = useNavigate();
  const tabs = mobileTabItems(items);
  const currentRoute = findRoute(location.pathname);
  const isTabRoot =
    tabs.some((tab) => tab.path === location.pathname) || location.pathname === MORE_PATH;

  return (
    <div className="app-shell app-shell-mobile">
      <header className="mobile-topbar">
        <div className="mobile-topbar-lead">
          {isTabRoot ? null : (
            <Button
              aria-label="返回"
              className="mobile-back"
              size="icon"
              variant="ghost"
              onClick={() => navigate(-1)}
            >
              <ChevronLeft size={22} />
            </Button>
          )}
          {/* Chrome, not content: the page keeps the only `h1`, which on a
              detail screen names the record rather than the section. */}
          <span className="mobile-topbar-title">{currentRoute?.title ?? "工作台"}</span>
        </div>
        <LaboratorySheet />
      </header>

      <main className="page-scroll mobile-scroll">
        <Outlet />
      </main>

      <nav className="tabbar" aria-label="主导航">
        {tabs.map((tab) => (
          <TabLink icon={<tab.icon size={21} aria-hidden="true" />} key={tab.path} to={tab.path}>
            {tab.title}
          </TabLink>
        ))}
        <TabLink icon={<Ellipsis size={21} aria-hidden="true" />} to={MORE_PATH}>
          更多
        </TabLink>
      </nav>
    </div>
  );
}

function TabLink({
  children,
  icon,
  to,
}: {
  children: ReactNode;
  icon: ReactNode;
  to: string;
}) {
  return (
    <NavLink
      className={({ isActive }) => (isActive ? "tabbar-link active" : "tabbar-link")}
      // Detail pages belong to their list's tab, so `/inventory/:id` keeps
      // "库存" lit; the dashboard would otherwise match everything.
      end={to === "/dashboard"}
      to={to}
    >
      {icon}
      <span className="tabbar-label">{children}</span>
    </NavLink>
  );
}

/**
 * Laboratory switcher for the phone shell.
 *
 * Every page is scoped to the selected laboratory, so it has to stay visible
 * rather than live behind the "更多" tab — but a full-width select in a 44px
 * bar is unreadable, hence a chip that opens a sheet.
 */
function LaboratorySheet() {
  const {
    canSelectLaboratory,
    federationTrusts,
    laboratories,
    selectedLaboratoryName,
    selectedScopeValue,
    setSelectedScopeValue,
  } = useLaboratorySelection();

  const options = [
    ...laboratories.map((laboratory) => ({
      label: laboratory.name,
      scope: "本地",
      value: `local:${laboratory.laboratory_id}`,
    })),
    ...federationTrusts.map((trust) => ({
      label: federationTrustLabel(trust),
      scope: "联邦",
      value: laboratoryScopeKey(
        remoteLaboratoryScope(trust.remote_node_id, trust.remote_laboratory_id),
      ),
    })),
  ];

  if (!canSelectLaboratory) {
    return selectedLaboratoryName ? (
      <span className="mobile-lab-static" title={selectedLaboratoryName}>
        {selectedLaboratoryName}
      </span>
    ) : null;
  }

  return (
    <Dialog.Root>
      <Dialog.Trigger asChild>
        <Button className="mobile-lab-chip" variant="ghost">
          <Building2 size={15} aria-hidden="true" />
          <span className="mobile-lab-chip-name">{selectedLaboratoryName ?? "选择实验室"}</span>
        </Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content sheet-content">
          <div className="dialog-header">
            <Dialog.Title className="dialog-title">选择实验室</Dialog.Title>
          </div>
          <div className="dialog-body">
            <ul className="sheet-list">
              {options.map((option) => (
                <li key={option.value}>
                  <Dialog.Close asChild>
                    <button
                      className={
                        option.value === selectedScopeValue
                          ? "sheet-option active"
                          : "sheet-option"
                      }
                      type="button"
                      onClick={() => setSelectedScopeValue(option.value)}
                    >
                      <span className="sheet-option-text">
                        <span className="sheet-option-label">{option.label}</span>
                        <span className="sheet-option-meta">{option.scope}</span>
                      </span>
                      {option.value === selectedScopeValue ? <Check size={17} /> : null}
                    </button>
                  </Dialog.Close>
                </li>
              ))}
            </ul>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
