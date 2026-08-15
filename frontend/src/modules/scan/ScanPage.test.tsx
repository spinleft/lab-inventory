import { screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { beforeEach, describe, expect, it } from "vitest";
import { buildScanPayload } from "../../shared/lib/qrPayload";
import {
  LAB_CHEMISTRY_ID,
  LOCAL_NODE_ID,
  LOCAL_WEB_ORIGIN,
  REMOTE_LAB_ID,
  REMOTE_NODE_ID,
  testFederationTrust,
  testRegularUser,
} from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

const ASSET_ID = "00000000-0000-4000-8000-0000000000e1";
const ITEM_ID = "00000000-0000-4000-8000-0000000000e2";

/**
 * URLs the detail pages asked for after a scan resolved.
 *
 * This is what the tests actually assert on: whether a code resolved to the
 * local scope or to the federation proxy is visible in the request path and
 * nowhere else.
 */
let requestedUrls: string[] = [];

/**
 * Answers every detail fetch with a 404.
 *
 * The pages then render their "not found" state, which is enough to prove the
 * navigation happened without needing a full asset fixture.
 */
function captureDetailRequests() {
  const capture = ({ request }: { request: Request }) => {
    requestedUrls.push(new URL(request.url).pathname);
    return new HttpResponse(null, { status: 404 });
  };

  server.use(
    http.get("*/api/v1/local/assets/:assetId", capture),
    http.get("*/api/v1/local/inventory-items/:itemId", capture),
    http.get(
      "*/api/v1/federation/nodes/:nodeId/laboratories/:laboratoryId/assets/:assetId",
      capture,
    ),
    http.get(
      "*/api/v1/federation/nodes/:nodeId/laboratories/:laboratoryId/inventory-items/:itemId",
      capture,
    ),
  );
}

/** Makes the laboratory switcher see the seeded federation trust. */
function withFederationTrust() {
  server.use(
    http.get("*/api/v1/local/federation/trusts", () =>
      HttpResponse.json([testFederationTrust]),
    ),
  );
}

/** The in-app route a scanned payload produces, as MemoryRouter wants it. */
function scanRoute(
  nodeId: string,
  laboratoryId: string,
  type: "asset" | "item",
  id: string,
) {
  const url = new URL(
    buildScanPayload(LOCAL_WEB_ORIGIN, {
      laboratoryId,
      nodeId,
      resourceId: id,
      type,
    }),
  );
  return `${url.pathname}${url.search}`;
}

beforeEach(() => {
  requestedUrls = [];
});

describe("ScanPage", () => {
  it("opens a local asset when the code names this instance", async () => {
    signIn(testRegularUser);
    withFederationTrust();
    captureDetailRequests();

    renderRoute([scanRoute(LOCAL_NODE_ID, LAB_CHEMISTRY_ID, "asset", ASSET_ID)]);

    await waitFor(() => {
      expect(requestedUrls).toContain(`/api/v1/local/assets/${ASSET_ID}`);
    });
    expect(await screen.findByText("未找到资产")).toBeInTheDocument();
  });

  it("opens a local inventory item when the code points at one", async () => {
    signIn(testRegularUser);
    withFederationTrust();
    captureDetailRequests();

    renderRoute([scanRoute(LOCAL_NODE_ID, LAB_CHEMISTRY_ID, "item", ITEM_ID)]);

    await waitFor(() => {
      expect(requestedUrls).toContain(`/api/v1/local/inventory-items/${ITEM_ID}`);
    });
  });

  it("follows a partner laboratory's code out through the federation proxy", async () => {
    signIn(testRegularUser);
    withFederationTrust();
    captureDetailRequests();

    // A label printed by the remote instance carries its node id, not ours.
    renderRoute([scanRoute(REMOTE_NODE_ID, REMOTE_LAB_ID, "asset", ASSET_ID)]);

    await waitFor(() => {
      expect(requestedUrls).toContain(
        `/api/v1/federation/nodes/${REMOTE_NODE_ID}/laboratories/${REMOTE_LAB_ID}/assets/${ASSET_ID}`,
      );
    });
    // It must not have been read from the local laboratory instead.
    expect(requestedUrls).not.toContain(`/api/v1/local/assets/${ASSET_ID}`);
  });

  it("refuses a code from a laboratory there is no trust with", async () => {
    signIn(testRegularUser);
    captureDetailRequests();
    // No trusts configured, so the remote node is a stranger.

    renderRoute([scanRoute(REMOTE_NODE_ID, REMOTE_LAB_ID, "asset", ASSET_ID)]);

    expect(await screen.findByText(/尚未建立联邦互信/)).toBeInTheDocument();
    // Failing closed means nothing was fetched under any scope.
    expect(requestedUrls).toEqual([]);
  });

  it("reports a code that is not one of ours", async () => {
    signIn(testRegularUser);
    withFederationTrust();
    captureDetailRequests();

    renderRoute(["/scan?v=1&n=not-a-uuid&l=x&t=asset&i=y"]);

    expect(await screen.findByText(/无法识别这个二维码/)).toBeInTheDocument();
    expect(requestedUrls).toEqual([]);
  });

  it("stays put and offers manual entry when opened with no code", async () => {
    signIn(testRegularUser);
    withFederationTrust();

    renderRoute(["/scan"]);

    expect(await screen.findByLabelText("手动输入二维码内容")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /使用摄像头扫码/ })).toBeInTheDocument();
  });

  // jsdom exposes no camera, so this is also what a browser on a plain-HTTP LAN
  // address sees: the API is simply absent. The message has to point at that
  // rather than at a denied permission.
  it("explains that an insecure context has no camera", async () => {
    signIn(testRegularUser);
    withFederationTrust();

    const { user } = renderRoute(["/scan"]);

    await user.click(await screen.findByRole("button", { name: /使用摄像头扫码/ }));

    expect(await screen.findByText(/不是安全上下文/)).toBeInTheDocument();
  });

  it("resolves a payload pasted into the manual field", async () => {
    signIn(testRegularUser);
    withFederationTrust();
    captureDetailRequests();

    const { user } = renderRoute(["/scan"]);

    const input = await screen.findByLabelText("手动输入二维码内容");
    await user.click(input);
    await user.paste(
      buildScanPayload(LOCAL_WEB_ORIGIN, {
        laboratoryId: LAB_CHEMISTRY_ID,
        nodeId: LOCAL_NODE_ID,
        resourceId: ASSET_ID,
        type: "asset",
      }),
    );
    // Exact: the sidebar and command palette carry several "打开…" entries.
    await user.click(screen.getByRole("button", { name: "打开" }));

    await waitFor(() => {
      expect(requestedUrls).toContain(`/api/v1/local/assets/${ASSET_ID}`);
    });
  });
});
