import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:18081/api/v1";
const appUrl = "/admin/asset-parameters";
const laboratoryId = "20000000-0000-4000-8000-000000000001";
const millimeterUnitId = "20000000-0000-4000-8000-000000000101";

const currentUser = {
  email: "root@example.com",
  laboratory: null,
  user_id: "20000000-0000-4000-8000-000000000011",
  user_type: {
    name: "root",
    user_type_id: "20000000-0000-4000-8000-000000000012",
  },
  username: "root",
};

test("manage asset parameters grouped by unit dimension", async ({ page }) => {
  let postedParameter: Record<string, unknown> | null = null;
  let parameters = [
    {
      code: "length",
      created_at: "2026-06-23T09:00:00Z",
      data_type: "number",
      default_unit_id: millimeterUnitId,
      description: "Length parameter",
      is_archived: false,
      laboratory_id: laboratoryId,
      name: "长度",
      options: [],
      parameter_type_id: "20000000-0000-4000-8000-000000000201",
      unit_dimension: "length",
      updated_at: "2026-06-23T09:00:00Z",
    },
    {
      code: "serial_no",
      created_at: "2026-06-23T09:05:00Z",
      data_type: "text",
      default_unit_id: null,
      description: null,
      is_archived: false,
      laboratory_id: laboratoryId,
      name: "序列号",
      options: [],
      parameter_type_id: "20000000-0000-4000-8000-000000000202",
      unit_dimension: null,
      updated_at: "2026-06-23T09:05:00Z",
    },
    {
      code: "wavelength_range",
      created_at: "2026-06-23T09:10:00Z",
      data_type: "range",
      default_unit_id: millimeterUnitId,
      description: "Visible spectrum",
      is_archived: false,
      laboratory_id: laboratoryId,
      name: "波长范围",
      options: [],
      parameter_type_id: "20000000-0000-4000-8000-000000000203",
      unit_dimension: "length",
      updated_at: "2026-06-23T09:10:00Z",
    },
  ];

  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({ json: currentUser });
  });
  await page.route("**/api/v1/laboratories", async (route) => {
    await route.fulfill({
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
    });
  });
  await page.route("**/api/v1/units", async (route) => {
    await route.fulfill({
      json: [
        {
          allow_decimal: true,
          code: "mm",
          created_at: "2026-06-23T08:00:00Z",
          dimension: "length",
          name: "毫米",
          scale_to_base: 0.001,
          symbol: "mm",
          unit_id: millimeterUnitId,
        },
        {
          allow_decimal: false,
          code: "pcs",
          created_at: "2026-06-23T08:00:00Z",
          dimension: "count",
          name: "件",
          scale_to_base: 1,
          symbol: "pcs",
          unit_id: "20000000-0000-4000-8000-000000000102",
        },
      ],
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/asset-parameters`, async (route) => {
    if (route.request().method() === "POST") {
      postedParameter = route.request().postDataJSON() as Record<string, unknown>;
      parameters = [
        {
          ...postedParameter,
          created_at: "2026-06-23T10:00:00Z",
          laboratory_id: laboratoryId,
          options: postedParameter.options ?? [],
          parameter_type_id: "20000000-0000-4000-8000-000000000299",
          updated_at: "2026-06-23T10:00:00Z",
        } as (typeof parameters)[number],
        ...parameters,
      ];
      await route.fulfill({ status: 201, json: parameters[0] });
      return;
    }

    await route.fulfill({ json: parameters });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto(appUrl);
  await expect(page.getByRole("heading", { exact: true, name: "资产参数" })).toBeVisible();
  await expect(page.locator(".asset-parameter-groups-table")).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "长度" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "无单位维度" })).toBeVisible();
  await expect(page.getByText("序列号")).toBeVisible();
  await expect(page.getByText("波长范围")).toBeVisible();
  await expectAlignedParameterRows(page);
  await expectNoHorizontalOverflow(page);

  await page.getByRole("button", { name: "新建参数" }).click();
  await expect(page.getByRole("heading", { name: "新建资产参数" })).toBeVisible();
  await page.getByLabel("数据类型").click();
  await page.getByRole("option", { name: "范围" }).click();
  await page.getByLabel("参数名称").fill("厚度范围");
  await page.getByLabel("参数代码").fill("thickness_range");
  await page.getByRole("button", { name: "保存" }).click();

  await expect.poll(() => postedParameter).toMatchObject({
    code: "thickness_range",
    data_type: "range",
    default_unit_id: null,
    is_archived: false,
    name: "厚度范围",
    options: [],
    unit_dimension: "length",
  });
  await expect(page.getByText("厚度范围")).toBeVisible();
  await expectAlignedParameterRows(page);
  await expectNoHorizontalOverflow(page);
});

async function expectAlignedParameterRows(page: Page) {
  const columnLefts = await page.locator(".asset-parameter-groups-table").evaluate((table) =>
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
