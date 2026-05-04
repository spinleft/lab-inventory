import { type CurrentUser } from "./types";

type UserLike = Pick<CurrentUser, "user_id" | "user_type" | "laboratory">;

export function isAdminUser(user: UserLike | null | undefined) {
  const name = user?.user_type.name;
  return name === "owner" || name === "admin";
}

export function isMaintainerUser(user: UserLike | null | undefined) {
  return user?.user_type.name === "maintainer";
}

export function canAccessAdminArea(user: UserLike | null | undefined) {
  return isAdminUser(user) || isMaintainerUser(user);
}

export function canManageLaboratories(user: UserLike | null | undefined) {
  return isAdminUser(user);
}

export function userTypeLabel(value: string) {
  switch (value) {
    case "owner":
    case "admin":
      return "管理员";
    case "maintainer":
      return "实验室维护者";
    case "user":
      return "普通用户";
    case "guest":
      return "访客";
    default:
      return value;
  }
}

export function userTypeBadgeClass(value: string) {
  switch (value) {
    case "owner":
    case "admin":
      return "badge badge-admin";
    case "maintainer":
      return "badge badge-maintainer";
    default:
      return "badge";
  }
}
