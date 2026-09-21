import { expect, test } from "@playwright/test";
import { isPhoneShell } from "./shell";

const apiBaseUrl = "http://127.0.0.1:18081/api/v1";
const laboratoryId = "50000000-0000-4000-8000-000000000001";
const unitId = "50000000-0000-4000-8000-000000000201";

const currentUser = {
  email: "root@example.com",
  laboratory: null,
  user_id: "50000000-0000-4000-8000-000000000011",
  user_type: {
    name: "root",
    user_type_id: "50000000-0000-4000-8000-000000000012",
  },
  username: "root",
};

/** Far more parameters than fit on one screen, which is what the picker caps. */
const parameters = Array.from({ length: 30 }, (_, index) => {
  const position = index + 1;

  return {
    code: `param_${position}`,
    created_at: "2026-06-23T08:00:00Z",
    data_type: "text",
    default_unit_id: null,
    description: null,
    laboratory_id: laboratoryId,
    name: `参数 ${position}`,
    options: [],
    parameter_type_id: `50000000-0000-4000-8000-0000000${String(position).padStart(5, "0")}`,
    unit_dimension: null,
    updated_at: "2026-06-23T08:00:00Z",
  };
});

test("the extra parameter picker scrolls instead of running off screen", async ({ page }) => {
  await page.route("**/api/v1/health_check", (route) =>
    route.fulfill({ status: 200, body: "OK" }),
  );
  await page.route("**/api/v1/auth/me", (route) => route.fulfill({ json: currentUser }));
  await page.route("**/api/v1/admin/laboratories", (route) =>
    route.fulfill({
      json: [
        {
          address: "Building A",
          contact: null,
          created_at: "2026-06-23T08:00:00Z",
          description: null,
          laboratory_id: laboratoryId,
          name: "中心实验室",
          updated_at: "2026-06-23T08:00:00Z",
        },
      ],
    }),
  );
  await page.route(`**/api/v1/admin/laboratories/${laboratoryId}/asset-categories`, (route) =>
    route.fulfill({ json: [] }),
  );
  await page.route(`**/api/v1/admin/laboratories/${laboratoryId}/asset-parameters`, (route) =>
    route.fulfill({ json: parameters }),
  );
  await page.route(`**/api/v1/admin/laboratories/${laboratoryId}/locations`, (route) =>
    route.fulfill({ json: [] }),
  );
  await page.route(`**/api/v1/admin/laboratories/${laboratoryId}/units`, (route) =>
    route.fulfill({
      json: [
        {
          allow_decimal: false,
          code: "pcs",
          created_at: "2026-06-23T08:00:00Z",
          dimension: "count",
          name: "件",
          scale_to_base: 1,
          symbol: "pcs",
          unit_id: unitId,
        },
      ],
    }),
  );
  await page.route(`**/api/v1/admin/laboratories/${laboratoryId}/assets**`, (route) =>
    route.fulfill({ json: { items: [], limit: 20, offset: 0, total: 0 } }),
  );
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/assets");
  await expect(page.getByRole("heading", { exact: true, name: "资产" })).toBeVisible();
  test.skip(await isPhoneShell(page), "手机外壳的表单是抽屉式的，这个用例针对桌面外壳");

  await page.getByRole("button", { name: "添加资产" }).click();
  await expect(page.getByRole("dialog", { name: "添加资产" })).toBeVisible();

  await page.getByLabel("添加额外参数").click();

  const dropdown = page.locator(".select-content");
  await expect(dropdown).toBeVisible();
  const viewport = page.locator(".select-viewport");
  // Capped and scrollable: everything past the cap is still reachable.
  expect(await viewport.evaluate((node) => node.scrollHeight > node.clientHeight + 1)).toBe(
    true,
  );

  const viewportHeight = page.viewportSize()?.height ?? 0;
  const box = await dropdown.boundingBox();
  expect(box).not.toBeNull();
  expect(Math.round(box!.y)).toBeGreaterThanOrEqual(0);
  expect(Math.round(box!.y + box!.height)).toBeLessThanOrEqual(viewportHeight);

  // And the last entry can still be picked.
  const lastOption = page.getByRole("option", { name: "参数 30 (param_30)" });
  await lastOption.scrollIntoViewIfNeeded();
  await lastOption.click();
  await expect(page.getByLabel("添加额外参数")).toContainText("参数 30");
});
