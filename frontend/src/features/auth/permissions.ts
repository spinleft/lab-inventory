import { type CurrentUser } from "./types";

export function getUserTypeName(user: CurrentUser) {
  return user.user_type.name;
}

export function canAccessUserSettings(_user: CurrentUser) {
  return true;
}

export function canAccessSystemSettings(user: CurrentUser) {
  return getUserTypeName(user) === "owner";
}

export function canAccessAdminSettings(user: CurrentUser) {
  return ["owner", "maintainer"].includes(getUserTypeName(user));
}

export function describeRole(user: CurrentUser) {
  const roleName = getUserTypeName(user);
  if (roleName === "owner") {
    return "系统所有者";
  }
  if (roleName === "maintainer") {
    return "实验室维护者";
  }
  if (roleName === "user") {
    return "实验室用户";
  }
  return roleName;
}

export function describeScope(user: CurrentUser) {
  if (getUserTypeName(user) === "owner") {
    return "全部实验室";
  }
  return user.laboratory?.name ?? "未绑定实验室";
}
