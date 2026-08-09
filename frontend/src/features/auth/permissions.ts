import { type CurrentUser } from "./types";

export function getUserTypeName(user: CurrentUser) {
  return user.user_type.name;
}

export function canAccessUserSettings(_user: CurrentUser) {
  return true;
}

export function canAccessSystemSettings(user: CurrentUser) {
  const typeName = getUserTypeName(user);
  return typeName === "super_admin" || typeName === "root";
}

export function canAccessAdminSettings(user: CurrentUser) {
  const typeName = getUserTypeName(user);
  return typeName === "super_admin" || typeName === "lab_admin" || typeName === "root";
}

export function describeRole(user: CurrentUser) {
  const roleName = getUserTypeName(user);
  if (roleName === "root") {
    return "系统超级用户";
  }
  if (roleName === "super_admin") {
    return "超级管理员";
  }
  if (roleName === "lab_admin") {
    return "实验室管理员";
  }
  if (roleName === "user") {
    return "用户";
  }
  if (roleName === "guest") {
    return "访客";
  }
  return roleName;
}

export function describeScope(user: CurrentUser) {
  if (getUserTypeName(user) === "root" || getUserTypeName(user) === "super_admin") {
    return "全部实验室";
  }
  if (getUserTypeName(user) === "guest") {
    return "访客";
  }
  return user.laboratory?.name ?? "未绑定实验室";
}
