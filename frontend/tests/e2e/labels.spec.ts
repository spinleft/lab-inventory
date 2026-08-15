import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:18086/api/v1";
const laboratoryId = "10000000-0000-4000-8000-000000000010";
const localNodeId = "10000000-0000-4000-8000-0000000000d1";
const remoteNodeId = "10000000-0000-4000-8000-0000000000b1";
const remoteLaboratoryId = "10000000-0000-4000-8000-0000000000b2";
const assetId = "10000000-0000-4000-8000-0000000000e1";
const printerId = "10000000-0000-4000-8000-0000000000f1";

const currentUser = {
  email: "lab-user@example.com",
  laboratory: { laboratory_id: laboratoryId, name: "化学实验室" },
  user_id: "00000000-0000-4000-8000-000000000004",
  user_type: {
    name: "user",
    user_type_id: "00000000-0000-4000-8000-000000000024",
  },
  username: "lab-user",
};

const printer = {
  auto_cut: true,
  created_at: "2026-08-16T08:00:00Z",
  host: "192.168.1.50",
  laboratory_id: laboratoryId,
  layout: {
    dpi: 300,
    max_length_dots: 11811,
    min_length_dots: 150,
    printable_length_dots: 271,
    printable_width_dots: 696,
  },
  media_kind: "die_cut",
  media_length_mm: 29,
  media_width_mm: 62,
  model: "QL-820NWBc",
  name: "前台标签机",
  port: 9100,
  printer_id: printerId,
  updated_at: "2026-08-16T08:00:00Z",
};

const asset = {
  asset_id: assetId,
  category_id: null,
  created_at: "2026-08-16T08:00:00Z",
  internal_notes: null,
  inventory_items: [],
  inventory_summary: { item_count: 0, quantity_allocated: 0, quantity_on_hand: 0 },
  inventory_unit_id: "10000000-0000-4000-8000-000000000001",
  laboratory_id: laboratoryId,
  manufacturer: "示例厂商",
  model: "XR-200",
  name: "低温冰箱",
  parameters: [],
  public_notes: null,
  tracking_mode: "serialized",
  updated_at: "2026-08-16T08:00:00Z",
};

const federationTrust = {
  created_at: "2026-08-16T08:00:00Z",
  local_laboratory_id: laboratoryId,
  remote_base_url: "https://remote.example.com/api/v1",
  remote_laboratory_id: remoteLaboratoryId,
  remote_laboratory_name: "远端实验室",
  remote_node_id: remoteNodeId,
  revoked_at: null,
  status: "active",
  trust_id: "10000000-0000-4000-8000-0000000000b3",
  updated_at: "2026-08-16T08:00:00Z",
};

/** Wires up the routes every test here needs, plus the configured API origin. */
async function stubBackend(page: Page) {
  await page.route("**/api/v1/auth/me", (route) => route.fulfill({ json: currentUser }));
  await page.route("**/api/v1/instance-identity", (route) =>
    route.fulfill({
      json: { node_id: localNodeId, public_web_url: "http://127.0.0.1:5173" },
    }),
  );
  await page.route("**/api/v1/local/federation/trusts", (route) =>
    route.fulfill({ json: [federationTrust] }),
  );
  await page.route("**/api/v1/local/label-printers", (route) =>
    route.fulfill({ json: [printer] }),
  );
  await page.route(`**/api/v1/local/label-printers/${printerId}/status`, (route) =>
    route.fulfill({
      json: {
        faults: [],
        media_kind: "die_cut",
        media_length_mm: 29,
        media_matches_configuration: true,
        media_width_mm: 62,
        ready: true,
      },
    }),
  );
  await page.route(`**/api/v1/local/assets/${assetId}`, (route) =>
    route.fulfill({ json: asset }),
  );
  // Collections the shell and detail page fetch alongside the asset.
  await page.route("**/api/v1/local/*", async (route) => {
    if (route.request().method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill({ json: [] });
  });

  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);
}

test("scanning a local label opens the asset it points at", async ({ page }) => {
  await stubBackend(page);

  await page.goto(
    `/scan?v=1&n=${localNodeId}&l=${laboratoryId}&t=asset&i=${assetId}`,
  );

  await expect(page.getByRole("heading", { name: "低温冰箱" })).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/assets/${assetId}$`));
});

test("scanning a label from an untrusted laboratory is refused", async ({ page }) => {
  await stubBackend(page);
  // A node this instance has no trust with.
  const strangerNodeId = "10000000-0000-4000-8000-0000000000c9";

  await page.goto(
    `/scan?v=1&n=${strangerNodeId}&l=${remoteLaboratoryId}&t=asset&i=${assetId}`,
  );

  await expect(page.getByText(/尚未建立联邦互信/)).toBeVisible();
  await expect(page).toHaveURL(/\/scan\?/);
});

test("printing a label sends a packed bitmap to the chosen printer", async ({ page }) => {
  await stubBackend(page);

  type PrintRequest = {
    copies?: number;
    pages?: { bitmap_base64: string; height_dots: number; width_dots: number }[];
  };
  let printRequest: PrintRequest | undefined;

  await page.route(`**/api/v1/local/label-printers/${printerId}/print`, async (route) => {
    printRequest = route.request().postDataJSON() as PrintRequest;
    await route.fulfill({ json: { labels_printed: 2 } });
  });

  await page.goto(`/assets/${assetId}`);
  await page.getByRole("button", { name: "打印标签" }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("前台标签机", { exact: false })).toBeVisible();

  // A preview of the real bitmap is rendered before anything is sent.
  await expect(dialog.locator("canvas")).toBeVisible();

  await dialog.getByLabel("每项份数").fill("2");
  await dialog.getByRole("button", { name: /打印 2 张/ }).click();

  await expect(page.getByText("已发送打印")).toBeVisible();

  expect(printRequest).toBeDefined();
  expect(printRequest?.copies).toBe(2);
  expect(printRequest?.pages).toHaveLength(1);

  const firstPage = printRequest?.pages?.[0];
  // The bitmap must match the printer's printable area exactly, and carry one
  // bit per dot; anything else is refused by the server.
  expect(firstPage?.width_dots).toBe(696);
  expect(firstPage?.height_dots).toBe(271);
  const decoded = Buffer.from(firstPage?.bitmap_base64 ?? "", "base64");
  expect(decoded).toHaveLength(Math.ceil(696 / 8) * 271);
  // A blank label would be all zeroes; the QR code and text must have marked it.
  expect(decoded.some((byte) => byte !== 0)).toBe(true);
});
