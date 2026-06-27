import { describe, expect, it } from "vitest";
import { federationTrustSchema, pairingCodeSchema } from "./api";
import {
  assetDetailPath,
  inventoryItemDetailPath,
  laboratoryCollectionPath,
  remoteLaboratoryScope,
} from "./scope";

const REMOTE_NODE_ID = "00000000-0000-4000-8000-000000000101";
const REMOTE_LABORATORY_ID = "00000000-0000-4000-8000-000000000102";

describe("federation api schemas", () => {
  it("parses federation trust responses", () => {
    expect(
      federationTrustSchema.parse({
        created_at: "2026-06-27T00:00:00Z",
        local_laboratory_id: "00000000-0000-4000-8000-000000000001",
        remote_base_url: "https://10.0.0.12:8000",
        remote_laboratory_id: REMOTE_LABORATORY_ID,
        remote_laboratory_name: "Remote Lab",
        remote_node_id: REMOTE_NODE_ID,
        revoked_at: null,
        status: "active",
        trust_id: "00000000-0000-4000-8000-000000000103",
        updated_at: "2026-06-27T00:00:00Z",
      }).remote_laboratory_name,
    ).toBe("Remote Lab");
  });

  it("parses pairing code responses", () => {
    expect(
      pairingCodeSchema.parse({
        expires_at: "2026-06-27T00:15:00Z",
        local_base_url: "https://10.0.0.10:8000",
        local_laboratory_id: "00000000-0000-4000-8000-000000000001",
        local_node_id: "00000000-0000-4000-8000-000000000002",
        pairing_code: "pairing-secret",
        pairing_code_id: "00000000-0000-4000-8000-000000000003",
      }).pairing_code,
    ).toBe("pairing-secret");
  });
});

describe("federation scope paths", () => {
  it("builds remote proxy paths for collection and detail reads", () => {
    const scope = remoteLaboratoryScope(REMOTE_NODE_ID, REMOTE_LABORATORY_ID);

    expect(laboratoryCollectionPath(scope, "assets")).toBe(
      `/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LABORATORY_ID}/assets`,
    );
    expect(assetDetailPath(scope, "00000000-0000-4000-8000-000000000201")).toBe(
      `/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LABORATORY_ID}/assets/00000000-0000-4000-8000-000000000201`,
    );
    expect(inventoryItemDetailPath(scope, "00000000-0000-4000-8000-000000000301")).toBe(
      `/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LABORATORY_ID}/inventory-items/00000000-0000-4000-8000-000000000301`,
    );
  });
});
