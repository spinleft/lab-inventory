import { describe, expect, it } from "vitest";
import {
  canAccessAdmin,
  canAccessAuditLogs,
  canManageAssetCategories,
  canManageLocations,
  canManageUnits,
  canSelectAssetCategoryLaboratory,
  canSelectLocationLaboratory,
  describeScope,
  roleLabel,
} from "./permissions";
import { type CurrentUser } from "./types";

function user(role: CurrentUser["user_type"]["name"], laboratory = null): CurrentUser {
  return {
    email: null,
    laboratory,
    user_id: "00000000-0000-4000-8000-000000000001",
    user_type: {
      name: role,
      user_type_id: "00000000-0000-4000-8000-000000000002",
    },
    username: role,
  };
}

describe("permissions", () => {
  it("allows admin navigation for admin roles only", () => {
    expect(canAccessAdmin(user("root"))).toBe(true);
    expect(canAccessAdmin(user("super_admin"))).toBe(true);
    expect(canAccessAdmin(user("lab_admin"))).toBe(true);
    expect(canAccessAdmin(user("user"))).toBe(false);
  });

  it("limits audit logs to root and super admins", () => {
    expect(canAccessAuditLogs(user("root"))).toBe(true);
    expect(canAccessAuditLogs(user("super_admin"))).toBe(true);
    expect(canAccessAuditLogs(user("lab_admin"))).toBe(false);
  });

  it("allows asset category management for scoped users but not guests", () => {
    expect(canManageAssetCategories(user("root"))).toBe(true);
    expect(canManageAssetCategories(user("super_admin"))).toBe(true);
    expect(canManageAssetCategories(user("lab_admin"))).toBe(true);
    expect(canManageAssetCategories(user("user"))).toBe(true);
    expect(canManageAssetCategories(user("guest"))).toBe(false);
  });

  it("allows location management for scoped users but not guests", () => {
    expect(canManageLocations(user("root"))).toBe(true);
    expect(canManageLocations(user("super_admin"))).toBe(true);
    expect(canManageLocations(user("lab_admin"))).toBe(true);
    expect(canManageLocations(user("user"))).toBe(true);
    expect(canManageLocations(user("guest"))).toBe(false);
  });

  it("limits unit management to server admins", () => {
    expect(canManageUnits(user("root"))).toBe(true);
    expect(canManageUnits(user("super_admin"))).toBe(true);
    expect(canManageUnits(user("lab_admin"))).toBe(false);
    expect(canManageUnits(user("user"))).toBe(false);
    expect(canManageUnits(user("guest"))).toBe(false);
  });

  it("limits asset category laboratory selection to global admins", () => {
    expect(canSelectAssetCategoryLaboratory(user("root"))).toBe(true);
    expect(canSelectAssetCategoryLaboratory(user("super_admin"))).toBe(true);
    expect(canSelectAssetCategoryLaboratory(user("lab_admin"))).toBe(false);
    expect(canSelectAssetCategoryLaboratory(user("user"))).toBe(false);
  });

  it("limits location laboratory selection to global admins", () => {
    expect(canSelectLocationLaboratory(user("root"))).toBe(true);
    expect(canSelectLocationLaboratory(user("super_admin"))).toBe(true);
    expect(canSelectLocationLaboratory(user("lab_admin"))).toBe(false);
    expect(canSelectLocationLaboratory(user("user"))).toBe(false);
  });

  it("formats role and scope labels", () => {
    expect(roleLabel("lab_admin")).toBe("实验室管理员");
    expect(describeScope(user("super_admin"))).toBe("全部实验室");
  });
});
