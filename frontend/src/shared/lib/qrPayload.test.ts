import { describe, expect, it } from "vitest";
import {
  buildScanPayload,
  parseScanPayload,
  scanTargetPath,
  type ScanTarget,
} from "./qrPayload";

const NODE_ID = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";
const LABORATORY_ID = "8a7b6c5d-4e3f-4a2b-9c8d-7e6f5a4b3c2d";
const RESOURCE_ID = "11111111-2222-4333-8444-555555555555";

const target: ScanTarget = {
  laboratoryId: LABORATORY_ID,
  nodeId: NODE_ID,
  resourceId: RESOURCE_ID,
  type: "asset",
};

describe("buildScanPayload", () => {
  it("produces a URL a browser can open", () => {
    const payload = buildScanPayload("https://lab.example.edu", target);
    const url = new URL(payload);
    expect(url.origin).toBe("https://lab.example.edu");
    expect(url.pathname).toBe("/scan");
    expect(url.searchParams.get("v")).toBe("1");
    expect(url.searchParams.get("n")).toBe(NODE_ID);
    expect(url.searchParams.get("l")).toBe(LABORATORY_ID);
    expect(url.searchParams.get("t")).toBe("asset");
    expect(url.searchParams.get("i")).toBe(RESOURCE_ID);
  });

  it("does not double up the slash when the origin has a trailing one", () => {
    const payload = buildScanPayload("https://lab.example.edu/", target);
    expect(payload).toContain("https://lab.example.edu/scan?");
  });
});

describe("parseScanPayload", () => {
  it("round-trips everything build produces", () => {
    for (const type of ["asset", "item"] as const) {
      const payload = buildScanPayload("https://lab.example.edu", {
        ...target,
        type,
      });
      expect(parseScanPayload(payload)).toEqual({ ...target, type });
    }
  });

  it("ignores the host, so a federated label still resolves", () => {
    // The label was printed by another instance; only the parameters matter.
    const payload = buildScanPayload("https://other-lab.example.org", target);
    expect(parseScanPayload(payload)).toEqual(target);
  });

  it("accepts a relative path or a bare query string", () => {
    const query = `v=1&n=${NODE_ID}&l=${LABORATORY_ID}&t=item&i=${RESOURCE_ID}`;
    expect(parseScanPayload(`/scan?${query}`)).toEqual({
      ...target,
      type: "item",
    });
    expect(parseScanPayload(`?${query}`)).toEqual({ ...target, type: "item" });
    expect(parseScanPayload(query)).toEqual({ ...target, type: "item" });
  });

  it("tolerates surrounding whitespace from a scanner", () => {
    const payload = buildScanPayload("https://lab.example.edu", target);
    expect(parseScanPayload(`  ${payload}\n`)).toEqual(target);
  });

  it("refuses a version it does not understand", () => {
    const payload = buildScanPayload("https://lab.example.edu", target).replace(
      "v=1",
      "v=2",
    );
    expect(parseScanPayload(payload)).toBeNull();
  });

  it("refuses payloads with a missing or malformed field", () => {
    const base = buildScanPayload("https://lab.example.edu", target);
    expect(parseScanPayload(base.replace(/&n=[^&]+/, ""))).toBeNull();
    expect(parseScanPayload(base.replace(/&l=[^&]+/, ""))).toBeNull();
    expect(parseScanPayload(base.replace(/&i=[^&]+/, ""))).toBeNull();
    expect(parseScanPayload(base.replace("t=asset", "t=location"))).toBeNull();
    expect(parseScanPayload(base.replace(NODE_ID, "not-a-uuid"))).toBeNull();
  });

  it("refuses things that are not payloads at all", () => {
    for (const value of [
      "",
      "   ",
      "hello world",
      "https://example.com/",
      RESOURCE_ID,
    ]) {
      expect(parseScanPayload(value)).toBeNull();
    }
  });
});

describe("scanTargetPath", () => {
  it("routes each target type to its detail page", () => {
    expect(scanTargetPath(target)).toBe(`/assets/${RESOURCE_ID}`);
    expect(scanTargetPath({ ...target, type: "item" })).toBe(
      `/inventory/${RESOURCE_ID}`,
    );
  });
});
