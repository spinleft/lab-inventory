import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:18081/api/v1";
const laboratoryId = "40000000-0000-4000-8000-000000000001";
const categoryId = "40000000-0000-4000-8000-000000000101";
const unitId = "40000000-0000-4000-8000-000000000201";
const locationRootId = "40000000-0000-4000-8000-000000000301";
const locationShelfId = "40000000-0000-4000-8000-000000000302";
const serializedAssetId = "40000000-0000-4000-8000-000000000401";
const quantityAssetId = "40000000-0000-4000-8000-000000000402";
const serializedItemId = "40000000-0000-4000-8000-000000000501";
const quantityItemId = "40000000-0000-4000-8000-000000000502";
const powerParameterId = "40000000-0000-4000-8000-000000000601";

const currentUser = {
  email: "root@example.com",
  laboratory: null,
  user_id: "40000000-0000-4000-8000-000000000011",
  user_type: {
    name: "root",
    user_type_id: "40000000-0000-4000-8000-000000000012",
  },
  username: "root",
};

type InventoryFixture = Record<string, unknown> & {
  inventory_item_id: string;
};

test("browse, filter, edit, create, delete, and open inventory detail", async ({ page }) => {
  let lastInventoryUrl: URL | null = null;
  let lastAssetsUrl: URL | null = null;
  let postedInventory: Record<string, unknown> | null = null;
  let patchedInventory: Record<string, unknown> | null = null;
  let deletedInventoryId = "";
  let inventoryItems: InventoryFixture[] = [serializedInventoryItem(), quantityInventoryItem()];

  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({ json: currentUser });
  });
  await page.route("**/api/v1/laboratories", async (route) => {
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
  await page.route(`**/api/v1/laboratories/${laboratoryId}/asset-categories`, async (route) => {
    await route.fulfill({
      json: [
        {
          category_id: categoryId,
          code: "microscope",
          created_at: "2026-06-24T08:00:00Z",
          depth: 0,
          description: null,
          laboratory_id: laboratoryId,
          name: "显微镜",
          parameter_assignments: [],
          parent_category_id: null,
          path: "microscope",
          updated_at: "2026-06-24T08:00:00Z",
        },
      ],
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/asset-parameters`, async (route) => {
    await route.fulfill({
      json: [
        {
          code: "power",
          created_at: "2026-06-24T08:00:00Z",
          data_type: "number",
          default_unit_id: unitId,
          description: null,
          is_archived: false,
          laboratory_id: laboratoryId,
          name: "功率",
          options: [],
          parameter_type_id: powerParameterId,
          unit_dimension: "count",
          updated_at: "2026-06-24T08:00:00Z",
        },
      ],
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/locations`, async (route) => {
    await route.fulfill({
      json: [
        {
          code: "room",
          created_at: "2026-06-24T08:00:00Z",
          depth: 0,
          description: null,
          laboratory_id: laboratoryId,
          location_id: locationRootId,
          name: "库房",
          parent_location_id: null,
          path: "room",
          updated_at: "2026-06-24T08:00:00Z",
        },
        {
          code: "shelf-a",
          created_at: "2026-06-24T08:00:00Z",
          depth: 1,
          description: null,
          laboratory_id: laboratoryId,
          location_id: locationShelfId,
          name: "A 架",
          parent_location_id: locationRootId,
          path: "room.shelf-a",
          updated_at: "2026-06-24T08:00:00Z",
        },
      ],
    });
  });
  await page.route("**/api/v1/units", async (route) => {
    await route.fulfill({
      json: [
        {
          allow_decimal: false,
          code: "pcs",
          created_at: "2026-06-24T08:00:00Z",
          dimension: "count",
          name: "件",
          scale_to_base: 1,
          symbol: "pcs",
          unit_id: unitId,
        },
      ],
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/inventory-items**`, async (route) => {
    lastInventoryUrl = new URL(route.request().url());
    await route.fulfill({
      json: {
        items: inventoryItems,
        limit: 30,
        offset: Number(lastInventoryUrl.searchParams.get("offset") ?? 0),
        total: inventoryItems.length,
      },
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/assets**`, async (route) => {
    lastAssetsUrl = new URL(route.request().url());
    await route.fulfill({
      json: {
        items: [assetDetail(serializedAssetId), assetDetail(quantityAssetId, "试剂", "quantity")],
        limit: 50,
        offset: Number(lastAssetsUrl.searchParams.get("offset") ?? 0),
        total: 2,
      },
    });
  });
  await page.route(`**/api/v1/assets/${serializedAssetId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/assets/${quantityAssetId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/inventory-items/${serializedItemId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/inventory-items/${quantityItemId}/attachments`, async (route) => {
    await route.fulfill({ json: [] });
  });
  await page.route(`**/api/v1/assets/${serializedAssetId}**`, async (route) => {
    await route.fulfill({ json: assetDetail(serializedAssetId) });
  });
  await page.route(`**/api/v1/assets/${quantityAssetId}**`, async (route) => {
    await route.fulfill({ json: assetDetail(quantityAssetId, "试剂", "quantity") });
  });
  await page.route(`**/api/v1/assets/${quantityAssetId}/inventory-items`, async (route) => {
    postedInventory = route.request().postDataJSON() as Record<string, unknown>;
    const created = {
      ...quantityInventoryItem("40000000-0000-4000-8000-000000000599"),
      batch_number: postedInventory.batch_number,
      quantity_on_hand: postedInventory.quantity_on_hand,
    };
    inventoryItems = [created, ...inventoryItems];
    await route.fulfill({ status: 201, json: [created] });
  });
  await page.route(`**/api/v1/inventory-items/${serializedItemId}`, async (route) => {
    if (route.request().method() === "PATCH") {
      patchedInventory = route.request().postDataJSON() as Record<string, unknown>;
      inventoryItems = inventoryItems.map((item) =>
        item.inventory_item_id === serializedItemId
          ? { ...item, ...patchedInventory, updated_at: "2026-06-24T11:00:00Z" }
          : item,
      );
      await route.fulfill({ json: inventoryItems.find((item) => item.inventory_item_id === serializedItemId) });
      return;
    }
    if (route.request().method() === "DELETE") {
      deletedInventoryId = serializedItemId;
      inventoryItems = inventoryItems.filter((item) => item.inventory_item_id !== serializedItemId);
      await route.fulfill({ status: 204, body: "" });
      return;
    }
    await route.fulfill({ json: inventoryItems.find((item) => item.inventory_item_id === serializedItemId) });
  });
  await page.route(`**/api/v1/inventory-items/${quantityItemId}`, async (route) => {
    await route.fulfill({ json: inventoryItems.find((item) => item.inventory_item_id === quantityItemId) });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/inventory");
  await expect(page.getByRole("heading", { exact: true, name: "库存" })).toBeVisible();
  await expect(page.getByText("SN-001")).toBeVisible();
  await expect(page.getByText("显微镜 A")).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByLabel("关键词").fill("SN-001");
  await page.getByLabel("库存状态").click();
  await page.getByRole("option", { name: "可用" }).click();
  await page.getByRole("button", { name: "应用筛选" }).click();
  await expect.poll(() => lastInventoryUrl?.searchParams.get("keyword")).toBe("SN-001");
  await expect.poll(() => lastInventoryUrl?.searchParams.get("status")).toBe("available");

  await page.getByRole("combobox", { exact: true, name: "位置" }).click();
  await page.getByRole("option", { name: /A 架/ }).click();
  await page.getByRole("button", { name: "应用筛选" }).click();
  await expect.poll(() => lastInventoryUrl?.searchParams.get("location_id")).toBe(locationShelfId);
  await page
    .getByRole("navigation", { name: "库存位置路径" })
    .getByRole("link", { name: "A 架" })
    .click();
  await expect(page).toHaveURL(new RegExp(`/assets\\?location_id=${locationShelfId}`));
  await expect.poll(() => lastAssetsUrl?.searchParams.get("location_id")).toBe(locationShelfId);

  await page.goto(`/assets/${serializedAssetId}`);
  await expect(page.getByRole("heading", { name: "显微镜 A" })).toBeVisible();
  await page.getByText("SN-001").first().click();
  await expect(page).toHaveURL(new RegExp(`/inventory/${serializedItemId}`));

  await page.goto("/inventory");
  await page.getByRole("tab", { name: "参数信息" }).click();
  await expect(page.getByRole("button", { name: "功率" })).toBeVisible();
  await expect(page.getByText("42 pcs").first()).toBeVisible();

  await page.getByText("SN-001").first().click();
  await expect(page).toHaveURL(new RegExp(`/inventory/${serializedItemId}`));
  await expect(page.getByRole("heading", { name: "SN-001" })).toBeVisible();
  await expect(page.getByText("查看资产")).toBeVisible();
  await expect(page.getByText("功率")).toBeVisible();
  await page.getByRole("button", { name: "编辑库存" }).click();
  await expect(page.getByRole("heading", { name: "编辑库存" })).toBeVisible();
  await page.locator("#inventory-editor-serial-number").fill("SN-DETAIL");
  await page.getByRole("button", { name: "保存" }).click();
  await expect.poll(() => patchedInventory).toMatchObject({ serial_number: "SN-DETAIL" });

  await page.goto("/inventory");
  await page.getByRole("button", { name: /库存 SN-DETAIL 操作/ }).click();
  await page.getByText("编辑库存").click();
  await expect(page.getByRole("heading", { name: "编辑库存" })).toBeVisible();
  await page.locator("#inventory-editor-serial-number").fill("SN-002");
  await page.getByRole("button", { name: "保存" }).click();
  await expect.poll(() => patchedInventory).toMatchObject({ serial_number: "SN-002" });

  await page.getByRole("button", { name: "添加库存" }).click();
  await expect(page.getByRole("heading", { name: "添加库存" })).toBeVisible();
  await page.getByRole("combobox", { exact: true, name: "资产" }).click();
  await page.getByRole("option", { name: /试剂/ }).click();
  await page.locator("#inventory-editor-quantity-on-hand").fill("12");
  await page.locator("#inventory-editor-batch").fill("B-99");
  await page.getByRole("button", { name: "保存" }).click();
  await expect.poll(() => postedInventory).toMatchObject({
    batch_number: "B-99",
    quantity_on_hand: 12,
  });

  await page.getByRole("button", { name: /库存 SN-002 操作/ }).click();
  await page.getByText("删除库存").click();
  await page.getByRole("button", { name: "删除" }).click();
  await expect.poll(() => deletedInventoryId).toBe(serializedItemId);
});

function serializedInventoryItem(): InventoryFixture {
  return {
    asset: {
      asset_id: serializedAssetId,
      category_id: categoryId,
      default_unit_id: unitId,
      manufacturer: "Olympus",
      model: "BX53",
      name: "显微镜 A",
    },
    asset_id: serializedAssetId,
    batch_number: null,
    created_at: "2026-06-24T09:00:00Z",
    internal_notes: null,
    inventory_item_id: serializedItemId,
    laboratory_id: laboratoryId,
    last_stocktake_at: null,
    location_id: locationShelfId,
    public_notes: null,
    quantity_allocated: 0,
    quantity_on_hand: 1,
    quantity_unit_id: unitId,
    serial_number: "SN-001",
    status: "available",
    tracking_mode: "serialized",
    updated_at: "2026-06-24T09:00:00Z",
  };
}

function quantityInventoryItem(id = quantityItemId): InventoryFixture {
  return {
    asset: {
      asset_id: quantityAssetId,
      category_id: categoryId,
      default_unit_id: unitId,
      manufacturer: null,
      model: "R-1",
      name: "试剂",
    },
    asset_id: quantityAssetId,
    batch_number: "B-42",
    created_at: "2026-06-24T09:05:00Z",
    internal_notes: null,
    inventory_item_id: id,
    laboratory_id: laboratoryId,
    last_stocktake_at: null,
    location_id: locationRootId,
    public_notes: null,
    quantity_allocated: 2,
    quantity_on_hand: 10,
    quantity_unit_id: unitId,
    serial_number: null,
    status: "reserved",
    tracking_mode: "quantity",
    updated_at: "2026-06-24T09:05:00Z",
  };
}

function assetDetail(assetId: string, name = "显微镜 A", trackingMode = "serialized") {
  return {
    asset_id: assetId,
    category_id: categoryId,
    created_at: "2026-06-24T08:30:00Z",
    default_unit_id: unitId,
    internal_notes: null,
    inventory_items:
      trackingMode === "serialized" ? [serializedInventoryItem()] : [quantityInventoryItem()],
    inventory_summary: {
      item_count: 1,
      quantity_allocated: 0,
      quantity_on_hand: 1,
    },
    is_archived: false,
    laboratory_id: laboratoryId,
    manufacturer: trackingMode === "serialized" ? "Olympus" : null,
    model: trackingMode === "serialized" ? "BX53" : "R-1",
    name,
    parameters: [
      {
        asset_id: assetId,
        code: "power",
        created_at: "2026-06-24T09:00:00Z",
        data_type: "number",
        default_unit_id: unitId,
        laboratory_id: laboratoryId,
        name: "功率",
        parameter_type_id: powerParameterId,
        unit_dimension: "count",
        updated_at: "2026-06-24T09:00:00Z",
        value: {
          number: 42,
          unit_id: unitId,
        },
        value_id: `${assetId.slice(0, 8)}-0000-4000-8000-000000000701`,
      },
    ],
    public_notes: null,
    tracking_mode: trackingMode,
    updated_at: "2026-06-24T08:30:00Z",
  };
}

async function expectNoHorizontalOverflow(page: Page) {
  await expect
    .poll(() =>
      page.evaluate(() => {
        const root = document.documentElement;
        return root.scrollWidth <= root.clientWidth + 1;
      }),
    )
    .toBe(true);
}
