import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:18081/api/v1";
const appUrl = "/admin/asset-categories";
const laboratoryId = "30000000-0000-4000-8000-000000000001";
const categoryId = "30000000-0000-4000-8000-000000000101";
const serialParameterId = "30000000-0000-4000-8000-000000000201";
const manufacturerParameterId = "30000000-0000-4000-8000-000000000202";

const currentUser = {
  email: "root@example.com",
  laboratory: null,
  user_id: "30000000-0000-4000-8000-000000000011",
  user_type: {
    name: "root",
    user_type_id: "30000000-0000-4000-8000-000000000012",
  },
  username: "root",
};

test("manage asset category parameter assignments", async ({ page }) => {
  let patchedCategory: Record<string, unknown> | null = null;
  let categories: Array<Record<string, unknown>> = [
    {
      category_id: categoryId,
      code: "microscopes",
      created_at: "2026-06-23T09:00:00Z",
      depth: 0,
      description: "Optical devices",
      laboratory_id: laboratoryId,
      name: "显微镜",
      parameter_assignments: [
        {
          applies_to_descendants: true,
          assignment_id: "30000000-0000-4000-8000-000000000301",
          is_required: true,
          parameter_type_id: serialParameterId,
          sort_order: 1,
        },
      ],
      parent_category_id: null,
      path: "microscopes",
      updated_at: "2026-06-23T09:00:00Z",
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
  await page.route(`**/api/v1/laboratories/${laboratoryId}/asset-parameters`, async (route) => {
    await route.fulfill({
      json: [
        {
          code: "serial_number",
          created_at: "2026-06-23T09:00:00Z",
          data_type: "text",
          default_unit_id: null,
          description: null,
          is_archived: false,
          laboratory_id: laboratoryId,
          name: "序列号",
          options: [],
          parameter_type_id: serialParameterId,
          unit_dimension: null,
          updated_at: "2026-06-23T09:00:00Z",
        },
        {
          code: "manufacturer",
          created_at: "2026-06-23T09:05:00Z",
          data_type: "text",
          default_unit_id: null,
          description: null,
          is_archived: false,
          laboratory_id: laboratoryId,
          name: "制造商",
          options: [],
          parameter_type_id: manufacturerParameterId,
          unit_dimension: null,
          updated_at: "2026-06-23T09:05:00Z",
        },
      ],
    });
  });
  await page.route(`**/api/v1/laboratories/${laboratoryId}/asset-categories`, async (route) => {
    await route.fulfill({ json: categories });
  });
  await page.route(`**/api/v1/asset-categories/${categoryId}`, async (route) => {
    patchedCategory = route.request().postDataJSON() as Record<string, unknown>;
    categories = [
      {
        ...categories[0],
        ...patchedCategory,
        parameter_assignments: (
          patchedCategory.parameter_assignments as Array<Record<string, unknown>>
        ).map((assignment, index) => ({
          ...assignment,
          assignment_id: `30000000-0000-4000-8000-00000000040${index}`,
        })),
        updated_at: "2026-06-23T10:00:00Z",
      },
    ];
    await route.fulfill({ json: categories[0] });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto(appUrl);
  await expect(page.getByRole("heading", { exact: true, name: "资产分类" })).toBeVisible();
  await expect(page.getByText("显微镜")).toBeVisible();
  await expect(page.locator(".category-tree-table").getByText("1 个", { exact: true })).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByRole("button", { name: "编辑" }).click();
  await expect(page.getByRole("heading", { name: "编辑资产分类" })).toBeVisible();
  await page.locator('input[id^="asset-category-parameter-sort-"]').first().fill("2");
  await page.getByRole("button", { name: "添加参数" }).click();
  await page.locator('input[id^="asset-category-parameter-sort-"]').nth(1).fill("1");
  await page.getByRole("button", { name: "删除附带参数" }).first().click();
  await page.getByRole("button", { name: "保存" }).click();

  await expect.poll(() => patchedCategory).toMatchObject({
    code: "microscopes",
    name: "显微镜",
    parameter_assignments: [
      {
        applies_to_descendants: true,
        is_required: true,
        parameter_type_id: manufacturerParameterId,
        sort_order: 1,
      },
    ],
  });
  await expect(page.getByText("制造商")).not.toBeVisible();
  await expect(page.locator(".category-tree-table").getByText("1 个", { exact: true })).toBeVisible();
  await expectNoHorizontalOverflow(page);
});

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
