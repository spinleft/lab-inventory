import {
  Activity,
  Boxes,
  Building2,
  FolderTree,
  Gauge,
  KeyRound,
  MapPin,
  PackageSearch,
  Ruler,
  ScrollText,
  Settings,
  SlidersHorizontal,
  UserRound,
  Users,
  type LucideIcon,
} from "lucide-react";
import { type ReactNode } from "react";
import { matchPath } from "react-router-dom";
import { AssetCategoriesPage } from "../modules/admin/AssetCategoriesPage";
import { AssetParametersPage } from "../modules/admin/AssetParametersPage";
import { AdminHomePage, LaboratoriesPage, UsersPage } from "../modules/admin/AdminPages";
import { LocationsPage } from "../modules/admin/LocationsPage";
import { UnitsPage } from "../modules/admin/UnitsPage";
import { AuditLogsPage } from "../modules/audit/AuditLogsPage";
import { AssetDetailPage } from "../modules/assets/AssetDetailPage";
import { AssetsPage } from "../modules/assets/AssetsPage";
import {
  canAccessAdmin,
  canAccessAssets,
  canAccessAuditLogs,
  canManageAssetCategories,
  canManageAssetParameters,
  canManageLocations,
  canManageUnits,
} from "../modules/auth/permissions";
import { type CurrentUser } from "../modules/auth/types";
import { DashboardPage } from "../modules/dashboard/DashboardPage";
import { InventoryDetailPage } from "../modules/inventory/InventoryDetailPage";
import { InventoryPage } from "../modules/inventory/InventoryPage";
import { PasswordPage, ProfilePage, PreferencesPage } from "../modules/profile/ProfilePages";

export type ModuleRoute = {
  canAccess?: (user: CurrentUser) => boolean;
  element: ReactNode;
  id: string;
  path: string;
  title: string;
};

export type ModuleNavItem = {
  canAccess?: (user: CurrentUser) => boolean;
  group: "workspace" | "admin" | "settings";
  icon: LucideIcon;
  path: string;
  title: string;
};

export type ModuleCommand = {
  canAccess?: (user: CurrentUser) => boolean;
  icon: LucideIcon;
  keywords?: string[];
  path: string;
  title: string;
};

export type FrontendModule = {
  commands: ModuleCommand[];
  id: string;
  navItems: ModuleNavItem[];
  routes: ModuleRoute[];
};

export const appModules: FrontendModule[] = [
  {
    id: "dashboard",
    navItems: [
      { group: "workspace", icon: Gauge, path: "/dashboard", title: "概览" },
    ],
    commands: [
      { icon: Gauge, path: "/dashboard", title: "打开概览", keywords: ["dashboard"] },
    ],
    routes: [
      { element: <DashboardPage />, id: "dashboard.index", path: "/dashboard", title: "概览" },
    ],
  },
  {
    id: "assets",
    navItems: [
      {
        canAccess: canAccessAssets,
        group: "workspace",
        icon: PackageSearch,
        path: "/assets",
        title: "资产",
      },
    ],
    commands: [
      {
        canAccess: canAccessAssets,
        icon: PackageSearch,
        path: "/assets",
        title: "查看资产",
        keywords: ["assets", "inventory"],
      },
    ],
    routes: [
      {
        canAccess: canAccessAssets,
        element: <AssetsPage />,
        id: "assets.index",
        path: "/assets",
        title: "资产",
      },
      {
        canAccess: canAccessAssets,
        element: <AssetDetailPage />,
        id: "assets.detail",
        path: "/assets/:assetId",
        title: "资产详情",
      },
    ],
  },
  {
    id: "inventory",
    navItems: [
      {
        canAccess: canAccessAssets,
        group: "workspace",
        icon: Boxes,
        path: "/inventory",
        title: "库存",
      },
    ],
    commands: [
      {
        canAccess: canAccessAssets,
        icon: Boxes,
        path: "/inventory",
        title: "查看库存",
        keywords: ["inventory", "stock"],
      },
    ],
    routes: [
      {
        canAccess: canAccessAssets,
        element: <InventoryPage />,
        id: "inventory.index",
        path: "/inventory",
        title: "库存",
      },
      {
        canAccess: canAccessAssets,
        element: <InventoryDetailPage />,
        id: "inventory.detail",
        path: "/inventory/:inventoryItemId",
        title: "库存详情",
      },
    ],
  },
  {
    id: "admin",
    navItems: [
      {
        canAccess: canAccessAdmin,
        group: "admin",
        icon: Building2,
        path: "/admin/laboratories",
        title: "实验室",
      },
      {
        canAccess: canAccessAdmin,
        group: "admin",
        icon: Users,
        path: "/admin/users",
        title: "用户",
      },
      {
        canAccess: canManageAssetCategories,
        group: "admin",
        icon: FolderTree,
        path: "/admin/asset-categories",
        title: "资产分类",
      },
      {
        canAccess: canManageAssetParameters,
        group: "admin",
        icon: SlidersHorizontal,
        path: "/admin/asset-parameters",
        title: "资产参数",
      },
      {
        canAccess: canManageLocations,
        group: "admin",
        icon: MapPin,
        path: "/admin/locations",
        title: "位置",
      },
      {
        canAccess: canManageUnits,
        group: "admin",
        icon: Ruler,
        path: "/admin/units",
        title: "单位",
      },
    ],
    commands: [
      {
        canAccess: canAccessAdmin,
        icon: Building2,
        path: "/admin/laboratories",
        title: "管理实验室",
      },
      {
        canAccess: canAccessAdmin,
        icon: Users,
        path: "/admin/users",
        title: "管理用户",
      },
      {
        canAccess: canManageAssetCategories,
        icon: FolderTree,
        path: "/admin/asset-categories",
        title: "管理资产分类",
      },
      {
        canAccess: canManageAssetParameters,
        icon: SlidersHorizontal,
        path: "/admin/asset-parameters",
        title: "管理资产参数",
      },
      {
        canAccess: canManageLocations,
        icon: MapPin,
        path: "/admin/locations",
        title: "管理位置",
      },
      {
        canAccess: canManageUnits,
        icon: Ruler,
        path: "/admin/units",
        title: "管理单位",
      },
    ],
    routes: [
      {
        canAccess: canManageAssetCategories,
        element: <AdminHomePage />,
        id: "admin.index",
        path: "/admin",
        title: "管理",
      },
      {
        canAccess: canAccessAdmin,
        element: <LaboratoriesPage />,
        id: "admin.laboratories",
        path: "/admin/laboratories",
        title: "实验室",
      },
      {
        canAccess: canAccessAdmin,
        element: <UsersPage />,
        id: "admin.users",
        path: "/admin/users",
        title: "用户",
      },
      {
        canAccess: canManageAssetCategories,
        element: <AssetCategoriesPage />,
        id: "admin.asset-categories",
        path: "/admin/asset-categories",
        title: "资产分类",
      },
      {
        canAccess: canManageAssetParameters,
        element: <AssetParametersPage />,
        id: "admin.asset-parameters",
        path: "/admin/asset-parameters",
        title: "资产参数",
      },
      {
        canAccess: canManageLocations,
        element: <LocationsPage />,
        id: "admin.locations",
        path: "/admin/locations",
        title: "位置",
      },
      {
        canAccess: canManageUnits,
        element: <UnitsPage />,
        id: "admin.units",
        path: "/admin/units",
        title: "单位",
      },
    ],
  },
  {
    id: "audit",
    navItems: [
      {
        canAccess: canAccessAuditLogs,
        group: "admin",
        icon: ScrollText,
        path: "/audit-logs",
        title: "审计日志",
      },
    ],
    commands: [
      {
        canAccess: canAccessAuditLogs,
        icon: ScrollText,
        path: "/audit-logs",
        title: "查看审计日志",
      },
    ],
    routes: [
      {
        canAccess: canAccessAuditLogs,
        element: <AuditLogsPage />,
        id: "audit.logs",
        path: "/audit-logs",
        title: "审计日志",
      },
    ],
  },
  {
    id: "profile",
    navItems: [],
    commands: [
      { icon: UserRound, path: "/settings/profile", title: "打开个人资料" },
      { icon: KeyRound, path: "/settings/password", title: "修改密码" },
      { icon: Settings, path: "/settings/preferences", title: "打开偏好设置" },
    ],
    routes: [
      {
        element: <ProfilePage />,
        id: "profile.index",
        path: "/settings/profile",
        title: "个人资料",
      },
      {
        element: <PasswordPage />,
        id: "profile.password",
        path: "/settings/password",
        title: "修改密码",
      },
      {
        element: <PreferencesPage />,
        id: "profile.preferences",
        path: "/settings/preferences",
        title: "偏好设置",
      },
    ],
  },
  {
    id: "server-settings",
    navItems: [],
    commands: [
      { icon: Activity, path: "/server-settings", title: "服务端设置" },
    ],
    routes: [],
  },
];

export const moduleRoutes = appModules.flatMap((module) => module.routes);
export const moduleNavItems = appModules.flatMap((module) => module.navItems);
export const moduleCommands = appModules.flatMap((module) => module.commands);

export function findRoute(pathname: string) {
  return moduleRoutes.find(
    (route) => route.path === pathname || matchPath({ end: true, path: route.path }, pathname),
  );
}
