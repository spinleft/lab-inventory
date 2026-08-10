import { type CurrentUser } from "../../modules/auth/types";
import { type Laboratory } from "../../modules/admin/api";
import { type FederationTrust } from "../../modules/federation/api";

const TIMESTAMP = "2026-06-17T00:00:00Z";

export const LAB_CHEMISTRY_ID = "00000000-0000-4000-8000-0000000000a1";
export const LAB_MATERIALS_ID = "00000000-0000-4000-8000-0000000000a2";
export const REMOTE_NODE_ID = "00000000-0000-4000-8000-0000000000b1";
export const REMOTE_LAB_ID = "00000000-0000-4000-8000-0000000000b2";

function user(overrides: {
  laboratory?: CurrentUser["laboratory"];
  typeName: CurrentUser["user_type"]["name"];
  userId: string;
  username: string;
}): CurrentUser {
  return {
    email: `${overrides.username}@example.com`,
    laboratory: overrides.laboratory ?? null,
    user_id: overrides.userId,
    user_type: {
      name: overrides.typeName,
      user_type_id: `00000000-0000-4000-8000-0000000000f${overrides.typeName.length}`,
    },
    username: overrides.username,
  };
}

const chemistryLaboratory = { laboratory_id: LAB_CHEMISTRY_ID, name: "化学实验室" };

/** root: global scope, sees every module including audit logs. */
export const testRootUser = user({
  typeName: "root",
  userId: "00000000-0000-4000-8000-000000000001",
  username: "root",
});

/** super_admin: global scope, same reach as root except for user management. */
export const testSuperAdminUser = user({
  typeName: "super_admin",
  userId: "00000000-0000-4000-8000-000000000002",
  username: "super-admin",
});

/** lab_admin: scoped to one laboratory, the only role that manages federation. */
export const testLabAdminUser = user({
  laboratory: chemistryLaboratory,
  typeName: "lab_admin",
  userId: "00000000-0000-4000-8000-000000000003",
  username: "lab-admin",
});

/** user: scoped to one laboratory, no admin surface. */
export const testRegularUser = user({
  laboratory: chemistryLaboratory,
  typeName: "user",
  userId: "00000000-0000-4000-8000-000000000004",
  username: "lab-user",
});

/** guest: laboratory-bound but read-only; reaches almost nothing. */
export const testGuestUser = user({
  laboratory: chemistryLaboratory,
  typeName: "guest",
  userId: "00000000-0000-4000-8000-000000000005",
  username: "lab-guest",
});

export const testLaboratories: Laboratory[] = [
  {
    address: "科研楼 A 座",
    contact: "chem@example.com",
    created_at: TIMESTAMP,
    description: "化学实验室",
    laboratory_id: LAB_CHEMISTRY_ID,
    name: "化学实验室",
    updated_at: TIMESTAMP,
  },
  {
    address: "科研楼 B 座",
    contact: null,
    created_at: TIMESTAMP,
    description: null,
    laboratory_id: LAB_MATERIALS_ID,
    name: "材料实验室",
    updated_at: TIMESTAMP,
  },
];

export const testFederationTrust: FederationTrust = {
  created_at: TIMESTAMP,
  local_laboratory_id: LAB_CHEMISTRY_ID,
  remote_base_url: "https://remote.example.com/api/v1",
  remote_laboratory_id: REMOTE_LAB_ID,
  remote_laboratory_name: "远端实验室",
  remote_node_id: REMOTE_NODE_ID,
  revoked_at: null,
  status: "active",
  trust_id: "00000000-0000-4000-8000-0000000000b3",
  updated_at: TIMESTAMP,
};
