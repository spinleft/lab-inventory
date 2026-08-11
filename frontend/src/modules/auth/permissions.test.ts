import { describe, expect, it } from "vitest";
import {
  canAccessAdmin,
  canAccessAssets,
  canAccessAuditLogs,
  canAccessBorrowRequests,
  canManageAssetCategories,
  canManageAssets,
  canManageFederation,
  canManageLaboratories,
  canManageLaboratoryAssets,
  canManageAssetParameters,
  canManageLocations,
  canManageUnits,
  canManageUser,
  canRequestBorrow,
  canSelectAssetLaboratory,
  canSelectAssetQueryLaboratory,
  canSelectAssetCategoryLaboratory,
  canSelectAssetParameterLaboratory,
  canSelectLocationLaboratory,
  describeRole,
  describeScope,
  getCreatableRoles,
  getUserTypeName,
  isAdmin,
  isLabAdmin,
  isRoot,
  isSuperAdmin,
  isSystemAdmin,
  roleLabel,
  roleRequiresLaboratory,
  roleTone,
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

describe("role predicates", () => {
  it("identifies each individual role", () => {
    expect(getUserTypeName(user("guest"))).toBe("guest");
    expect(isRoot(user("root"))).toBe(true);
    expect(isRoot(user("super_admin"))).toBe(false);
    expect(isSuperAdmin(user("super_admin"))).toBe(true);
    expect(isSuperAdmin(user("root"))).toBe(false);
    expect(isLabAdmin(user("lab_admin"))).toBe(true);
    expect(isLabAdmin(user("user"))).toBe(false);
  });

  it("treats root and super admin as system administrators", () => {
    expect(isSystemAdmin(user("root"))).toBe(true);
    expect(isSystemAdmin(user("super_admin"))).toBe(true);
    expect(isSystemAdmin(user("lab_admin"))).toBe(false);
    expect(isSystemAdmin(user("user"))).toBe(false);
  });

  it("treats laboratory admins as administrators but not system administrators", () => {
    expect(isAdmin(user("root"))).toBe(true);
    expect(isAdmin(user("lab_admin"))).toBe(true);
    expect(isAdmin(user("user"))).toBe(false);
    expect(isAdmin(user("guest"))).toBe(false);
  });
});

describe("asset and borrowing access", () => {
  it("requires a laboratory for scoped roles to reach assets", () => {
    expect(canAccessAssets(user("root"))).toBe(true);
    expect(canAccessAssets(user("super_admin"))).toBe(true);
    expect(canAccessAssets(user("lab_admin", ownLaboratory))).toBe(true);
    expect(canAccessAssets(user("guest", ownLaboratory))).toBe(true);
    expect(canAccessAssets(user("lab_admin"))).toBe(false);
    expect(canAccessAssets(user("guest"))).toBe(false);
  });

  it("mirrors category management for asset writes", () => {
    expect(canManageAssets(user("user"))).toBe(true);
    expect(canManageAssets(user("guest"))).toBe(false);
  });

  it("limits asset laboratory selection to system administrators", () => {
    expect(canSelectAssetLaboratory(user("root"))).toBe(true);
    expect(canSelectAssetLaboratory(user("lab_admin", ownLaboratory))).toBe(false);
  });

  it("limits federation management to laboratory admins with a laboratory", () => {
    expect(canManageFederation(user("lab_admin", ownLaboratory))).toBe(true);
    expect(canManageFederation(user("lab_admin"))).toBe(false);
    expect(canManageFederation(user("root"))).toBe(false);
    expect(canManageFederation(user("user", ownLaboratory))).toBe(false);
  });

  it("limits the borrow queue to laboratory members", () => {
    expect(canAccessBorrowRequests(user("lab_admin", ownLaboratory))).toBe(true);
    expect(canAccessBorrowRequests(user("user", ownLaboratory))).toBe(true);
    expect(canAccessBorrowRequests(user("guest", ownLaboratory))).toBe(false);
    expect(canAccessBorrowRequests(user("root"))).toBe(false);
    expect(canAccessBorrowRequests(user("user"))).toBe(false);
  });

  it("lets guests raise borrow requests but not root", () => {
    expect(canRequestBorrow(user("guest", ownLaboratory))).toBe(true);
    expect(canRequestBorrow(user("lab_admin", ownLaboratory))).toBe(true);
    expect(canRequestBorrow(user("user", ownLaboratory))).toBe(true);
    expect(canRequestBorrow(user("guest"))).toBe(false);
    expect(canRequestBorrow(user("root"))).toBe(false);
  });

  it("limits laboratory management to system administrators", () => {
    expect(canManageLaboratories(user("root"))).toBe(true);
    expect(canManageLaboratories(user("super_admin"))).toBe(true);
    expect(canManageLaboratories(user("lab_admin", ownLaboratory))).toBe(false);
  });
});

describe("canManageUser", () => {
  const target = (
    role: CurrentUser["user_type"]["name"],
    laboratory: CurrentUser["laboratory"] = null,
    userId = "target-id",
  ) => ({
    laboratory,
    user_id: userId,
    user_type: { name: role },
  });

  it("always lets an actor manage their own account", () => {
    const actor = user("guest", ownLaboratory);
    expect(canManageUser(actor, target("guest", ownLaboratory, actor.user_id))).toBe(true);
  });

  it("lets root manage anyone", () => {
    expect(canManageUser(user("root"), target("root"))).toBe(true);
    expect(canManageUser(user("root"), target("super_admin"))).toBe(true);
  });

  it("stops a super admin from managing root", () => {
    expect(canManageUser(user("super_admin"), target("root"))).toBe(false);
    expect(canManageUser(user("super_admin"), target("lab_admin"))).toBe(true);
  });

  it("confines a laboratory admin to their own laboratory members", () => {
    const actor = user("lab_admin", ownLaboratory);
    expect(canManageUser(actor, target("user", ownLaboratory))).toBe(true);
    expect(canManageUser(actor, target("root", ownLaboratory))).toBe(false);
    expect(canManageUser(actor, target("super_admin", ownLaboratory))).toBe(false);
    expect(
      canManageUser(actor, target("user", { laboratory_id: "other", name: "Other" })),
    ).toBe(false);
    expect(canManageUser(user("lab_admin"), target("user", ownLaboratory))).toBe(false);
  });

  it("denies plain users and guests", () => {
    expect(canManageUser(user("user", ownLaboratory), target("user", ownLaboratory))).toBe(
      false,
    );
    expect(canManageUser(user("guest", ownLaboratory), target("guest", ownLaboratory))).toBe(
      false,
    );
  });
});

describe("role presentation", () => {
  it("labels every known role and passes unknown ones through", () => {
    expect(roleLabel("root")).toBe("系统根用户");
    expect(roleLabel("super_admin")).toBe("超级管理员");
    expect(roleLabel("user")).toBe("普通用户");
    expect(roleLabel("guest")).toBe("访客");
    expect(roleLabel("something_else")).toBe("something_else");
  });

  it("tones roles by severity", () => {
    expect(roleTone("root")).toBe("danger");
    expect(roleTone("super_admin")).toBe("danger");
    expect(roleTone("lab_admin")).toBe("warning");
    expect(roleTone("user")).toBe("success");
    expect(roleTone("guest")).toBe("default");
  });

  it("describes the role of a user", () => {
    expect(describeRole(user("lab_admin", ownLaboratory))).toBe("实验室管理员");
  });

  it("describes scope for every role shape", () => {
    expect(describeScope(user("root"))).toBe("全部实验室");
    expect(describeScope(user("guest", ownLaboratory))).toBe("Own Lab");
    expect(describeScope(user("guest"))).toBe("访客");
    expect(describeScope(user("user", ownLaboratory))).toBe("Own Lab");
    expect(describeScope(user("user"))).toBe("未绑定实验室");
  });

  it("lists creatable roles per actor", () => {
    expect(getCreatableRoles(user("root"))).toEqual([
      "super_admin",
      "lab_admin",
      "user",
      "guest",
    ]);
    expect(getCreatableRoles(user("super_admin"))).toEqual([
      "super_admin",
      "lab_admin",
      "user",
      "guest",
    ]);
    expect(getCreatableRoles(user("lab_admin", ownLaboratory))).toEqual([
      "lab_admin",
      "user",
      "guest",
    ]);
    expect(getCreatableRoles(user("user", ownLaboratory))).toEqual([]);
  });

  it("knows which roles must be bound to a laboratory", () => {
    expect(roleRequiresLaboratory("lab_admin")).toBe(true);
    expect(roleRequiresLaboratory("user")).toBe(true);
    expect(roleRequiresLaboratory("guest")).toBe(true);
    expect(roleRequiresLaboratory("root")).toBe(false);
    expect(roleRequiresLaboratory("super_admin")).toBe(false);
  });
});
