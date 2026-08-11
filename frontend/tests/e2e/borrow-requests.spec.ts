import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:8000/api/v1";
const laboratoryId = "50000000-0000-4000-8000-000000000001";
const categoryId = "50000000-0000-4000-8000-000000000101";
const unitId = "50000000-0000-4000-8000-000000000201";
const locationId = "50000000-0000-4000-8000-000000000301";
const assetId = "50000000-0000-4000-8000-000000000401";
const inventoryItemId = "50000000-0000-4000-8000-000000000501";
const borrowRequestId = "50000000-0000-4000-8000-000000000601";

test("guest can request borrow from inventory detail", async ({ page }) => {
  const currentUser = {
    email: null,
    laboratory: { laboratory_id: laboratoryId, name: "中心实验室" },
    user_id: "50000000-0000-4000-8000-000000000011",
    user_type: {
      name: "guest",
      user_type_id: "50000000-0000-4000-8000-000000000012",
    },
    username: "guest-a",
  };
  let postedBorrowRequest: Record<string, unknown> | null = null;

  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({ json: currentUser });
  });
  await page.route(`**/api/v1/local/inventory-items/${inventoryItemId}`, async (route) => {
    await route.fulfill({ json: inventoryItem() });
  });
  await page.route(`**/api/v1/local/assets/${assetId}**`, async (route) => {
    await route.fulfill({ json: assetDetail() });
  });
  await page.route("**/api/v1/local/asset-categories", async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route("**/api/v1/local/asset-parameters", async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route("**/api/v1/local/locations", async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route("**/api/v1/local/units", async (route) => {
    await route.fulfill({ json: [unitFixture()] });
  });
  await page.route(`**/api/v1/local/assets/${assetId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/local/inventory-items/${inventoryItemId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/local/inventory-items/${inventoryItemId}/borrow-requests`, async (route) => {
    postedBorrowRequest = route.request().postDataJSON() as Record<string, unknown>;
    await route.fulfill({
      status: 201,
      json: borrowRequestFixture({
        request_note: postedBorrowRequest?.request_note as string | null,
      }),
    });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto(`/inventory/${inventoryItemId}`);
  await expect(page.getByRole("heading", { name: "SN-BORROW" })).toBeVisible();
  await expect(page.getByRole("button", { name: "申请借用" })).toBeVisible();

  await page.getByRole("button", { name: "申请借用" }).click();
  const dialog = page.getByRole("dialog", { name: "申请借用" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("备注").fill("需要做实验");
  await dialog.getByRole("button", { name: "提交申请" }).click();

  await expect.poll(() => postedBorrowRequest?.request_note).toBe("需要做实验");
  await expect(dialog).toBeHidden();
});

test("lab users can approve borrow requests", async ({ page }) => {
  const currentUser = {
    email: "user@example.com",
    laboratory: { laboratory_id: laboratoryId, name: "中心实验室" },
    user_id: "50000000-0000-4000-8000-000000000021",
    user_type: {
      name: "user",
      user_type_id: "50000000-0000-4000-8000-000000000022",
    },
    username: "lab-user",
  };
  let pendingRequests = [borrowRequestFixture()];
  let borrowedItems: Array<ReturnType<typeof inventoryItem>> = [];

  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({ json: currentUser });
  });
  await page.route("**/api/v1/admin/laboratories", async (route) => {
    await route.fulfill({
      json: [
        {
          address: "Building A",
          contact: null,
          created_at: "2026-06-24T08:00:00Z",
          description: null,
          laboratory_id: laboratoryId,
          name: "中心实验室",
          updated_at: "2026-06-24T08:00:00Z",
        },
      ],
    });
  });
  await page.route("**/api/v1/local/federation/trusts", async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route("**/api/v1/local/borrow-requests**", async (route) => {
    if (route.request().method() === "PATCH") {
      pendingRequests = [];
      borrowedItems = [inventoryItem({ status: "borrowed" })];
      await route.fulfill({
        json: borrowRequestFixture({ status: "approved", reviewed_by_user_id: currentUser.user_id, reviewed_by_username: currentUser.username, reviewed_by_user_type: currentUser.user_type.name }),
      });
      return;
    }

    await route.fulfill({ json: pendingRequests });
  });
  await page.route("**/api/v1/local/inventory-items", async (route) => {
    const url = new URL(route.request().url());
    if (url.searchParams.get("status") === "borrowed") {
      await route.fulfill({
        json: {
          items: borrowedItems,
          limit: 200,
          offset: 0,
          total: borrowedItems.length,
        },
      });
      return;
    }
    await route.fulfill({
      json: {
        items: [],
        limit: 30,
        offset: 0,
        total: 0,
      },
    });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/borrow-requests");
  await expect(page.getByRole("heading", { name: "借用管理" })).toBeVisible();
  await expect(page.getByText("Borrowable Reagent")).toBeVisible();
  await expect(page.getByText("SN-BORROW")).toBeVisible();

  await page.getByRole("button", { name: "批准" }).first().click();
  const dialog = page.getByRole("alertdialog", { name: "批准借用申请" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "批准" }).click();

  await expect.poll(() => pendingRequests.length).toBe(0);
  await expect(page.getByText("当前没有待审批的借用申请。")).toBeVisible();
  await expect.poll(() => borrowedItems[0]?.status).toBe("borrowed");
});

function borrowRequestFixture(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    asset_model: "BX53",
    asset_name: "Borrowable Reagent",
    borrow_request_id: borrowRequestId,
    created_at: "2026-06-24T09:10:00Z",
    decision_note: null,
    inventory_item_id: inventoryItemId,
    inventory_item_title: "SN-BORROW",
    inventory_status: "available",
    local_laboratory_id: laboratoryId,
    requester_guest_link_id: "50000000-0000-4000-8000-000000000701",
    requester_user_id: "50000000-0000-4000-8000-000000000011",
    requester_user_type: "guest",
    requester_username: "guest-a",
    request_note: "需要做实验",
    reviewed_at: null,
    reviewed_by_user_id: null,
    reviewed_by_user_type: null,
    reviewed_by_username: null,
    status: "pending",
    updated_at: "2026-06-24T09:10:00Z",
    ...overrides,
  };
}

function inventoryItem(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    asset: {
      asset_id: assetId,
      category_id: categoryId,
      inventory_unit_id: unitId,
      manufacturer: null,
      model: "BX53",
      name: "Borrowable Reagent",
    },
    asset_id: assetId,
    batch_number: "B-001",
    created_at: "2026-06-24T09:00:00Z",
    internal_notes: null,
    inventory_item_id: inventoryItemId,
    laboratory_id: laboratoryId,
    last_stocktake_at: null,
    location_id: locationId,
    public_notes: null,
    quantity_allocated: 0,
    quantity_on_hand: 1,
    serial_number: "SN-BORROW",
    status: "available",
    tracking_mode: "quantity",
    updated_at: "2026-06-24T09:00:00Z",
    ...overrides,
  };
}

function assetDetail() {
  return {
    asset_id: assetId,
    category_id: categoryId,
    created_at: "2026-06-24T08:30:00Z",
    inventory_unit_id: unitId,
    internal_notes: null,
    inventory_items: [inventoryItem()],
    inventory_summary: {
      item_count: 1,
      quantity_allocated: 0,
      quantity_on_hand: 1,
    },
    laboratory_id: laboratoryId,
    manufacturer: null,
    model: "BX53",
    name: "Borrowable Reagent",
    parameters: [],
    public_notes: null,
    tracking_mode: "quantity",
    updated_at: "2026-06-24T08:30:00Z",
  };
}

function unitFixture() {
  return {
    allow_decimal: false,
    code: "pcs",
    created_at: "2026-06-24T08:00:00Z",
    dimension: "count",
    name: "件",
    scale_to_base: 1,
    symbol: "pcs",
    unit_id: unitId,
  };
}
