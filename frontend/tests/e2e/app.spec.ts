import { expect, type Page, test } from "@playwright/test";

const apiBaseUrl = "http://127.0.0.1:8000/api/v1";
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

test("login, navigate, theme, audit, and logout", async ({ page }) => {
  let loggedIn = false;
  await page.route("**/api/v1/health_check", async (route) => {
    await route.fulfill({ status: 200, body: "OK" });
  });
  await page.route("**/api/v1/auth/login", async (route) => {
    loggedIn = true;
    await route.fulfill({ json: { message: "Login successful" } });
  });
  await page.route("**/api/v1/auth/me", async (route) => {
    if (!loggedIn) {
      await route.fulfill({ status: 401, json: { error: "Authentication required" } });
      return;
    }
    await route.fulfill({ json: currentUser });
  });
  await page.route("**/api/v1/auth/logout", async (route) => {
    loggedIn = false;
    await route.fulfill({ json: { message: "Logout successful" } });
  });
  await page.route("**/api/v1/laboratories", async (route) => {
    await route.fulfill({
      json: [
        {
          address: "A1",
          contact: null,
          created_at: "2026-06-17T00:00:00Z",
          description: null,
          laboratory_id: "00000000-0000-4000-8000-000000000011",
          name: "Main Lab",
          updated_at: "2026-06-17T00:00:00Z",
        },
      ],
    });
  });
  await page.route("**/api/v1/audit-logs**", async (route) => {
    await route.fulfill({
      json: {
        items: [
          {
            action: "create",
            actor_user_id: currentUser.user_id,
            actor_username: "root",
            audit_log_id: "00000000-0000-4000-8000-000000000021",
            created_at: "2026-06-17T00:00:00Z",
            details: { rollback: { operation: "delete" } },
            resource_id: "00000000-0000-4000-8000-000000000011",
            resource_type: "laboratory",
          },
        ],
        limit: 50,
        offset: 0,
        total: 1,
      },
    });
  });
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/login");
  await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
  await page.getByLabel("用户名").fill("root");
  await page.getByLabel("密码").fill("password");
  await page.getByRole("button", { name: "登录" }).click();

  await expect(page.getByRole("heading", { name: "概览" })).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.getByRole("button", { name: "切换主题" }).click();
  await page.getByText("深色").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  await page.getByRole("link", { name: "审计日志" }).first().click();
  await expect(page.getByRole("heading", { name: "审计日志" })).toBeVisible();
  await expect(page.getByText("laboratory")).toBeVisible();

  await openUserMenu(page);
  await page.getByText("退出登录").click();
  await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
});

async function openUserMenu(page: Page) {
  const userMenuButton = page.getByRole("button", { name: /用户菜单 root/ });
  if (await userMenuButton.isVisible().catch(() => false)) {
    await userMenuButton.click();
    return;
  }

  const mobileNavigationButton = page.getByRole("button", { name: "打开导航" });
  await mobileNavigationButton.click();
  await expect(userMenuButton).toBeVisible();
  await userMenuButton.click();
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
