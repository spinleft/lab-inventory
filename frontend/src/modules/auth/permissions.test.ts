import { describe, expect, it } from "vitest";
import {
  canAccessAdmin,
  canAccessAuditLogs,
  canManageAssetCategories,
  canManageLaboratoryAssets,
  canManageAssetParameters,
  canManageLocations,
  canManageUnits,
  canSelectAssetQueryLaboratory,
  canSelectAssetCategoryLaboratory,
  canSelectAssetParameterLaboratory,
  canSelectLocationLaboratory,
  describeScope,
  roleLabel,
} from "./permissions";
import { type CurrentUser } from "./types";

function user(
  role: CurrentUser["user_type"]["name"],
  laboratory: CurrentUser["laboratory"] = null,
): CurrentUser {
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

const ownLaboratory = {
  laboratory_id: "00000000-0000-4000-8000-000000000101",
  name: "Own Lab",
};

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

  it("allows asset parameter management for scoped users but not guests", () => {
    expect(canManageAssetParameters(user("root"))).toBe(true);
    expect(canManageAssetParameters(user("super_admin"))).toBe(true);
    expect(canManageAssetParameters(user("lab_admin"))).toBe(true);
    expect(canManageAssetParameters(user("user"))).toBe(true);
    expect(canManageAssetParameters(user("guest"))).toBe(false);
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

  it("allows scoped users to select laboratories for asset and inventory queries", () => {
    expect(canSelectAssetQueryLaboratory(user("root"))).toBe(true);
    expect(canSelectAssetQueryLaboratory(user("super_admin"))).toBe(true);
    expect(canSelectAssetQueryLaboratory(user("lab_admin", ownLaboratory))).toBe(true);
    expect(canSelectAssetQueryLaboratory(user("user", ownLaboratory))).toBe(true);
    expect(canSelectAssetQueryLaboratory(user("lab_admin"))).toBe(false);
    expect(canSelectAssetQueryLaboratory(user("user"))).toBe(false);
    expect(canSelectAssetQueryLaboratory(user("guest", ownLaboratory))).toBe(false);
  });

  it("allows asset writes only inside the actor laboratory unless globally scoped", () => {
    const otherLaboratoryId = "00000000-0000-4000-8000-000000000202";

    expect(canManageLaboratoryAssets(user("root"), otherLaboratoryId)).toBe(true);
    expect(canManageLaboratoryAssets(user("super_admin"), otherLaboratoryId)).toBe(true);
    expect(canManageLaboratoryAssets(user("lab_admin", ownLaboratory), ownLaboratory.laboratory_id)).toBe(true);
    expect(canManageLaboratoryAssets(user("user", ownLaboratory), ownLaboratory.laboratory_id)).toBe(true);
    expect(canManageLaboratoryAssets(user("lab_admin", ownLaboratory), otherLaboratoryId)).toBe(false);
    expect(canManageLaboratoryAssets(user("user", ownLaboratory), otherLaboratoryId)).toBe(false);
    expect(canManageLaboratoryAssets(user("guest", ownLaboratory), ownLaboratory.laboratory_id)).toBe(false);
  });

  it("limits asset parameter laboratory selection to global admins", () => {
    expect(canSelectAssetParameterLaboratory(user("root"))).toBe(true);
    expect(canSelectAssetParameterLaboratory(user("super_admin"))).toBe(true);
    expect(canSelectAssetParameterLaboratory(user("lab_admin"))).toBe(false);
    expect(canSelectAssetParameterLaboratory(user("user"))).toBe(false);
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
