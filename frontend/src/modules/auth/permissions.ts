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

export function isSystemAdmin(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
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

export function canAccessAssets(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user) || Boolean(user.laboratory);
}

export function canManageAssets(user: CurrentUser) {
  return canManageAssetCategories(user);
}

export function canSelectAssetLaboratory(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user);
}

export function canSelectAssetQueryLaboratory(user: CurrentUser) {
  return (
    isRoot(user) ||
    isSuperAdmin(user) ||
    ((isLabAdmin(user) || getUserTypeName(user) === "user") && Boolean(user.laboratory))
  );
}

export function canManageLaboratoryAssets(user: CurrentUser, laboratoryId?: string | null) {
  if (!laboratoryId || !canManageAssets(user)) {
    return false;
  }
  if (isRoot(user) || isSuperAdmin(user)) {
    return true;
  }
  return user.laboratory?.laboratory_id === laboratoryId;
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

export function canManageFederation(user: CurrentUser) {
  return isLabAdmin(user) && Boolean(user.laboratory);
}

/**
 * Registering a printer writes an address the server will dial, so it is
 * restricted the same way federation configuration is.
 */
export function canManageLabelPrinters(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user) || (isLabAdmin(user) && Boolean(user.laboratory));
}

/** Anyone who works in a laboratory may print a label; guests may not. */
export function canPrintLabels(user: CurrentUser) {
  const typeName = getUserTypeName(user);
  return (
    isRoot(user) ||
    isSuperAdmin(user) ||
    ((typeName === "lab_admin" || typeName === "user") && Boolean(user.laboratory))
  );
}

export function canAccessBorrowRequests(user: CurrentUser) {
  return (isLabAdmin(user) || getUserTypeName(user) === "user") && Boolean(user.laboratory);
}

export function canRequestBorrow(user: CurrentUser) {
  const typeName = getUserTypeName(user);
  return (typeName === "guest" || typeName === "lab_admin" || typeName === "user")
    && Boolean(user.laboratory);
}

/**
 * Borrowing from a remote laboratory goes out through the federation proxy,
 * which only laboratory administrators and users may use. A guest would be
 * refused by the backend, so they are not offered the action.
 */
export function canRequestRemoteBorrow(user: CurrentUser) {
  return (isLabAdmin(user) || getUserTypeName(user) === "user") && Boolean(user.laboratory);
}

/**
 * Anyone who can file a borrow request can see the ones they filed. That is a
 * wider audience than the review queue: guests are included, system admins are
 * not, because the routes behind it are laboratory-scoped.
 */
export function canViewMyBorrowRequests(user: CurrentUser) {
  return canRequestBorrow(user);
}

/**
 * Units belong to a laboratory, and its admin maintains them.
 *
 * Not the server admins alone — that left a laboratory admin unable to add a
 * unit for their own assets — and not every member either: a unit's scale
 * restates every quantity recorded against it. Mirrors `can_manage_units` in
 * the backend's access_control.rs.
 */
export function canManageUnits(user: CurrentUser) {
  return isRoot(user) || isSuperAdmin(user) || (isLabAdmin(user) && Boolean(user.laboratory));
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
