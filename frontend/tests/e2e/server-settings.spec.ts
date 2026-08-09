import { expect, type Page, test } from "@playwright/test";

test("user configures and tests a backend server", async ({ page }) => {
  await page.route("**/api/v1/health_check", async (route) => {
    await route.fulfill({
      headers: {
        "access-control-allow-credentials": "true",
        "access-control-allow-origin": "http://127.0.0.1:5173",
      },
      json: { status: "ok" },
    });
  });
  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({
      status: 401,
      headers: {
        "access-control-allow-credentials": "true",
        "access-control-allow-origin": "http://127.0.0.1:5173",
      },
      json: { error: "Authentication required" },
    });
  });

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "后端服务器设置" }),
  ).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByLabel("后端 API 地址").fill("http://127.0.0.1:8000");
  await page.getByRole("button", { name: "测试连接" }).click();

  await expect(page.getByText("连接正常。")).toBeVisible();
  await expect(page.getByLabel("后端 API 地址")).toHaveValue(
    "http://127.0.0.1:8000/api/v1",
  );
  await page.getByRole("button", { name: "继续" }).click();

  await expect(
    page.getByRole("heading", { exact: true, name: "登录" }),
  ).toBeVisible();
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
