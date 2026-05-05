import { expect, type Page, test } from "@playwright/test";

const corsHeaders = {
  "access-control-allow-credentials": "true",
  "access-control-allow-origin": "http://127.0.0.1:5173",
};

test("user logs in with backend credentials", async ({ page }) => {
  const currentUser = {
    user_id: "00000000-0000-4000-8000-000000000001",
    username: "admin",
    email: "admin@example.com",
    user_type: {
      user_type_id: "00000000-0000-4000-8000-000000000002",
      name: "owner",
    },
    laboratory: null,
  };
  await page.route("**/api/v1/auth/login", async (route) => {
    await route.fulfill({
      headers: corsHeaders,
      json: { message: "Login successful" },
    });
  });
  await page.route("**/api/v1/auth/me", async (route) => {
    await route.fulfill({
      headers: corsHeaders,
      json: currentUser,
    });
  });
  await page.route("**/api/v1/auth/logout", async (route) => {
    await route.fulfill({
      headers: corsHeaders,
      json: { message: "Logout successful" },
    });
  });
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "labInventory.apiBaseUrl",
      "http://127.0.0.1:8000/api/v1",
    );
  });

  await page.goto("/login");
  await expect(
    page.getByRole("heading", { exact: true, name: "登录" }),
  ).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByLabel("用户名").fill("root");
  await page.getByLabel("密码").fill("password");
  await page.getByRole("button", { name: "登录" }).click();

  await expect(page.getByRole("heading", { name: "概览" })).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByRole("button", { name: /admin/ }).click();
  await page.getByText("登出").click();
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
