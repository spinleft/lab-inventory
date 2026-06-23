import { type CurrentUser, type UserTypeName } from "./types";

export function getUserTypeName(user: CurrentUser) {
  return user.user_type.name;
}

export function isRoot(user: CurrentUser) {
  return getUserTypeName(user) === "root";
}

export function isSuperAdmin(user: CurrentUser) {
  return getUserTypeName(user) === "super_admin";
}

export function isLabAdmin(user: CurrentUser) {
  return getUserTypeName(user) === "lab_admin";
}

export function isAdmin(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user) || isLabAdmin(user);
}

export function canAccessAdmin(user: CurrentUser) {
  return isAdmin(user);
}

export function canAccessAuditLogs(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
}

export function canManageAssetCategories(user: CurrentUser) {
  return (
    isRoot(user) ||
    isSuperAdmin(user) ||
    isLabAdmin(user) ||
    getUserTypeName(user) === "user"
  );
}

export function canManageAssetParameters(user: CurrentUser) {
  return canManageAssetCategories(user);
}

export function canSelectAssetCategoryLaboratory(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
}

export function canSelectAssetParameterLaboratory(user: CurrentUser) {
  return canSelectAssetCategoryLaboratory(user);
}

export function canManageLocations(user: CurrentUser) {
  return canManageAssetCategories(user);
}

export function canManageUnits(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
}

export function canSelectLocationLaboratory(user: CurrentUser) {
  return canSelectAssetCategoryLaboratory(user);
}

export function canManageLaboratories(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
}

export function canManageUser(
  actor: CurrentUser,
  target: { laboratory?: { laboratory_id: string } | null; user_id?: string; user_type: { name: UserTypeName } },
) {
  if (actor.user_id === target.user_id) {
    return true;
  }
  if (isRoot(actor)) {
    return true;
  }
  if (isSuperAdmin(actor)) {
    return target.user_type.name !== "root";
  }
  if (isLabAdmin(actor)) {
    return (
      target.user_type.name !== "root" &&
      target.user_type.name !== "super_admin" &&
      !!actor.laboratory &&
      target.laboratory?.laboratory_id === actor.laboratory.laboratory_id
    );
  }
  return false;
}

export function roleLabel(roleName: UserTypeName | string) {
  const labels: Record<string, string> = {
    root: "系统根用户",
    super_admin: "超级管理员",
    lab_admin: "实验室管理员",
    user: "普通用户",
    guest: "访客",
  };
  return labels[roleName] ?? roleName;
}

export function roleTone(roleName: UserTypeName | string) {
  if (roleName === "root" || roleName === "super_admin") {
    return "danger" as const;
  }
  if (roleName === "lab_admin") {
    return "warning" as const;
  }
  if (roleName === "user") {
    return "success" as const;
  }
  return "default" as const;
}

export function describeRole(user: CurrentUser) {
  return roleLabel(getUserTypeName(user));
}

export function describeScope(user: CurrentUser) {
  if (isRoot(user) || isSuperAdmin(user)) {
    return "全部实验室";
  }
  if (getUserTypeName(user) === "guest") {
    return user.laboratory?.name ?? "访客";
  }
  return user.laboratory?.name ?? "未绑定实验室";
}

export function getCreatableRoles(actor: CurrentUser): UserTypeName[] {
  if (isRoot(actor) || isSuperAdmin(actor)) {
    return ["super_admin", "lab_admin", "user", "guest"];
  }
  if (isLabAdmin(actor)) {
    return ["lab_admin", "user", "guest"];
  }
  return [];
}

export function roleRequiresLaboratory(roleName: UserTypeName | string) {
  return roleName === "lab_admin" || roleName === "user" || roleName === "guest";
}
