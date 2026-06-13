import { type CurrentUser } from "./types";

export function getUserTypeName(user: CurrentUser) {
  return user.user_type.name;
}

export function canAccessUserSettings(_user: CurrentUser) {
  return true;
}

export function canAccessSystemSettings(user: CurrentUser) {
  return getUserTypeName(user) === "admin";
}

export function canAccessAdminSettings(user: CurrentUser) {
  return getUserTypeName(user) === "admin";
}

export function describeRole(user: CurrentUser) {
  const roleName = getUserTypeName(user);
  if (roleName === "admin") {
    return "管理员";
  }
  if (roleName === "user") {
    return "用户";
  }
  return roleName;
}

export function describeScope(user: CurrentUser) {
  if (getUserTypeName(user) === "admin") {
    return "本地节点";
  }
  return user.laboratory?.name ?? "未绑定实验室";
}
