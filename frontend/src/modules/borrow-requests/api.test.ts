import { describe, expect, it } from "vitest";
import { laboratoryCollectionPath, localLaboratoryScope, remoteLaboratoryScope } from "../federation/scope";
import { borrowRequestLabel, borrowRequestTone } from "./format";

const LABORATORY_ID = "00000000-0000-4000-8000-000000000001";
const REMOTE_NODE_ID = "00000000-0000-4000-8000-000000000101";
const REMOTE_LABORATORY_ID = "00000000-0000-4000-8000-000000000102";
const INVENTORY_ITEM_ID = "00000000-0000-4000-8000-0000000000c1";
const BORROW_REQUEST_ID = "00000000-0000-4000-8000-0000000000b1";

describe("borrow request paths", () => {
  it("files a local request against the laboratory-scoped route", () => {
    expect(
      laboratoryCollectionPath(
        localLaboratoryScope(LABORATORY_ID),
        `inventory-items/${INVENTORY_ITEM_ID}/borrow-requests`,
      ),
    ).toBe(`/local/inventory-items/${INVENTORY_ITEM_ID}/borrow-requests`);
  });

  it("files a remote request through the federation proxy", () => {
    expect(
      laboratoryCollectionPath(
        remoteLaboratoryScope(REMOTE_NODE_ID, REMOTE_LABORATORY_ID),
        `inventory-items/${INVENTORY_ITEM_ID}/borrow-requests`,
      ),
    ).toBe(
      `/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LABORATORY_ID}/inventory-items/${INVENTORY_ITEM_ID}/borrow-requests`,
    );
  });

  it("cancels through whichever laboratory holds the request", () => {
    expect(
      laboratoryCollectionPath(
        localLaboratoryScope(LABORATORY_ID),
        `borrow-requests/${BORROW_REQUEST_ID}/cancel`,
      ),
    ).toBe(`/local/borrow-requests/${BORROW_REQUEST_ID}/cancel`);
    expect(
      laboratoryCollectionPath(
        remoteLaboratoryScope(REMOTE_NODE_ID, REMOTE_LABORATORY_ID),
        `borrow-requests/${BORROW_REQUEST_ID}/cancel`,
      ),
    ).toBe(
      `/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LABORATORY_ID}/borrow-requests/${BORROW_REQUEST_ID}/cancel`,
    );
  });
});

describe("borrow request status formatting", () => {
  it("labels every status, including cancelled", () => {
    expect(borrowRequestLabel("pending")).toBe("待审批");
    expect(borrowRequestLabel("approved")).toBe("已批准");
    expect(borrowRequestLabel("rejected")).toBe("已拒绝");
    // Before `cancelled` existed the fallthrough labelled it 已拒绝, which told
    // a requester their own retraction had been refused.
    expect(borrowRequestLabel("cancelled")).toBe("已撤销");
  });

  it("does not tone a cancelled request as a refusal", () => {
    expect(borrowRequestTone("rejected")).toBe("danger");
    expect(borrowRequestTone("cancelled")).not.toBe("danger");
  });
});
