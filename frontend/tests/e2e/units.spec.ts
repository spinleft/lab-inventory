import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:18081/api/v1";
const appUrl = "/admin/units";

const currentUser = {
  email: "root@example.com",
  laboratory: null,
  user_id: "00000000-0000-4000-8000-000000000001",
  user_type: {
    name: "root",
    user_type_id: "00000000-0000-4000-8000-000000000002",
  },
  username: "root",
};

test("manage units", async ({ page }) => {
  let postedUnit: Record<string, unknown> | null = null;
  let units = [
    {
      allow_decimal: true,
      code: "mm",
      created_at: "2026-06-23T09:00:00Z",
      dimension: "length",
      name: "毫米",
      scale_to_base: 0.001,
      symbol: "mm",
      unit_id: "10000000-0000-4000-8000-000000000001",
    },
    {
      allow_decimal: false,
      code: "pcs",
      created_at: "2026-06-23T09:05:00Z",
      dimension: "count",
      name: "件",
      scale_to_base: 1,
      symbol: "pcs",
      unit_id: "10000000-0000-4000-8000-000000000002",
    },
  ];

  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({ json: currentUser });
  });
  await page.route("**/api/v1/units", async (route) => {
    if (route.request().method() === "POST") {
      postedUnit = route.request().postDataJSON() as Record<string, unknown>;
      units = [
        {
          ...postedUnit,
          created_at: "2026-06-23T10:00:00Z",
          unit_id: "10000000-0000-4000-8000-000000000099",
        } as (typeof units)[number],
        ...units,
      ];
      await route.fulfill({ status: 201, json: units[0] });
      return;
    }

    await route.fulfill({ json: units });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto(appUrl);
  await expect(page.getByRole("heading", { exact: true, name: "单位" })).toBeVisible();
  await expect(page.locator(".unit-groups-table")).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "长度" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "数量" })).toBeVisible();
  await expect(page.getByText("毫米")).toBeVisible();
  await expect(page.getByText("件")).toBeVisible();
  await expectAlignedUnitRows(page);
  await expectNoHorizontalOverflow(page);

  await page.getByRole("button", { name: "新建单位" }).click();
  await expect(page.getByRole("heading", { name: "新建单位" })).toBeVisible();
  await page.getByLabel("单位名称").fill("厘米");
  await page.getByLabel("单位维度").click();
  await expect(page.getByRole("option", { name: "长度" })).toBeVisible();
  await page.mouse.click(12, 12);
  await expect(page.getByRole("heading", { name: "新建单位" })).toBeVisible();
  await expect(page.getByLabel("单位名称")).toHaveValue("厘米");
  await expect(page.getByRole("option", { name: "长度" })).toBeHidden();
  await page.getByLabel("单位代码").fill("cm");
  await page.getByLabel("显示符号").fill("cm");
  await page.getByLabel("基础换算系数").fill("0.01");
  await page.getByRole("button", { name: "保存" }).click();

  await expect.poll(() => postedUnit).toMatchObject({
    allow_decimal: true,
    code: "cm",
    dimension: "length",
    name: "厘米",
    scale_to_base: 0.01,
    symbol: "cm",
  });
  await expect(page.getByText("厘米")).toBeVisible();
  await expectAlignedUnitRows(page);
  await expectNoHorizontalOverflow(page);
});

async function expectAlignedUnitRows(page: Page) {
  const columnLefts = await page.locator(".unit-groups-table").evaluate((table) =>
    Array.from(table.querySelectorAll("tbody tr:not(.unit-dimension-row)")).map((row) =>
      Array.from(row.children).map((cell) => Math.round(cell.getBoundingClientRect().left)),
    ),
  );

  expect(columnLefts.length).toBeGreaterThanOrEqual(2);
  for (const rowLefts of columnLefts.slice(1)) {
    expect(rowLefts).toEqual(columnLefts[0]);
  }
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
