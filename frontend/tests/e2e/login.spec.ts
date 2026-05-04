import { expect, test } from "@playwright/test";

test("user configures a backend server and logs in", async ({ page }) => {
  let isLoggedIn = false;

  await page.route("**/api/v1/health_check", async (route) => {
    await route.fulfill({ json: { status: "ok" } });
  });
  await page.route("**/api/v1/auth/login", async (route) => {
    isLoggedIn = true;
    await route.fulfill({ json: { message: "Login successful" } });
  });
  await page.route("**/api/v1/auth/me", async (route) => {
    if (!isLoggedIn) {
      await route.fulfill({
        status: 401,
        json: { error: "Authentication required" },
      });
      return;
    }

    await route.fulfill({
      json: {
        user_id: "00000000-0000-0000-0000-000000000001",
        username: "admin",
        email: "admin@example.com",
        user_type: {
          user_type_id: "00000000-0000-0000-0000-000000000002",
          name: "owner",
        },
        laboratory: null,
      },
    });
  });
  await page.route("**/api/v1/*-alerts", async (route) => {
    await route.fulfill({ json: [] });
  });

  await page.goto("/");
  await page.getByLabel("后端 API 地址").fill("http://127.0.0.1:8000");
  await page.getByRole("button", { name: "测试连接" }).click();
  await expect(page.getByText("连接正常。")).toBeVisible();
  await page.getByRole("button", { name: "继续" }).click();

  await page.getByLabel("用户名").fill("admin");
  await page.getByLabel("密码").fill("password");
  await page.getByRole("button", { name: "登录" }).click();

  await expect(page.getByRole("heading", { name: "概览" })).toBeVisible();
});
