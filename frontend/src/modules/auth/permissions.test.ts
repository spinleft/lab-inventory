import { describe, expect, it } from "vitest";
import { canAccessAdmin, canAccessAuditLogs, describeScope, roleLabel } from "./permissions";
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

  it("formats role and scope labels", () => {
    expect(roleLabel("lab_admin")).toBe("实验室管理员");
    expect(describeScope(user("super_admin"))).toBe("全部实验室");
  });
});
