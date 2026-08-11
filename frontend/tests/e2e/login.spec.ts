import { expect, type Page, test } from "@playwright/test";

const corsHeaders = {
  "access-control-allow-credentials": "true",
  "access-control-allow-origin": "http://127.0.0.1:5173",
};

test("user logs in with backend credentials", async ({ page }) => {
  let loggedIn = false;
  const currentUser = {
    user_id: "00000000-0000-4000-8000-000000000001",
    username: "root",
    email: "admin@example.com",
    user_type: {
      user_type_id: "00000000-0000-4000-8000-000000000002",
      name: "root",
    },
    laboratory: null,
  };
  await page.route("**/api/v1/auth/login", async (route) => {
    loggedIn = true;
    await route.fulfill({
      headers: corsHeaders,
      json: { message: "Login successful" },
    });
  });
  await page.route("**/api/v1/auth/me", async (route) => {
    if (!loggedIn) {
      await route.fulfill({
        status: 401,
        headers: corsHeaders,
        json: { error: "Authentication required" },
      });
      return;
    }
    await route.fulfill({
      headers: corsHeaders,
      json: currentUser,
    });
  });
  await page.route("**/api/v1/auth/logout", async (route) => {
    loggedIn = false;
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

  await page.getByRole("button", { name: /root/ }).click();
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
