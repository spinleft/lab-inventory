import { expect, type Page, test } from "@playwright/test";
import { openNavItem, setTheme, signOut } from "./shell";

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
  await page.route("**/api/v1/admin/laboratories", async (route) => {
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

  await setTheme(page, "深色");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  await openNavItem(page, "审计日志");
  await expect(page.getByRole("heading", { name: "审计日志" })).toBeVisible();
  await expect(page.getByText("laboratory")).toBeVisible();

  await signOut(page);
  await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
});

/**
 * The desktop shell is a fixed-height grid for the same reason the phone shell
 * is (see mobile-shell.spec.ts): with a `min-height` the row grows to fit the
 * page, the shell grows with it, and the sidebar scrolls out of view along with
 * the content. Scrolling belongs to .page-scroll.
 */
test("keeps the sidebar in place while a long page scrolls", async ({ page }) => {
  const auditLogs = Array.from({ length: 40 }, (_, index) => ({
    action: "create",
    actor_user_id: currentUser.user_id,
    actor_username: "root",
    audit_log_id: `00000000-0000-4000-8000-00000000${String(index).padStart(4, "0")}`,
    created_at: "2026-06-17T00:00:00Z",
    details: { rollback: { operation: "delete" } },
    resource_id: "00000000-0000-4000-8000-000000000011",
    resource_type: "laboratory",
  }));

  await page.route("**/api/v1/health_check", (route) => route.fulfill({ status: 200, body: "OK" }));
  await page.route("**/api/v1/auth/me", (route) => route.fulfill({ json: currentUser }));
  await page.route("**/api/v1/admin/laboratories", async (route) => {
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
  await page.route("**/api/v1/audit-logs**", (route) =>
    route.fulfill({
      json: { items: auditLogs, limit: 50, offset: 0, total: auditLogs.length },
    }),
  );
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/audit-logs");
  await expect(page.getByRole("heading", { exact: true, name: "审计日志" })).toBeVisible();

  const sidebar = page.locator(".sidebar");
  test.skip(!(await sidebar.isVisible()), "只有宽屏外壳才有常驻侧边栏");

  const before = await sidebar.boundingBox();
  expect(before).not.toBeNull();

  // The page itself must not scroll; the content area does.
  const documentScrolls = await page.evaluate(() => {
    const root = document.scrollingElement ?? document.documentElement;
    return root.scrollHeight > root.clientHeight + 1;
  });
  expect(documentScrolls).toBe(false);

  await page.locator(".page-scroll").evaluate((node) => node.scrollTo(0, 100_000));
  const after = await sidebar.boundingBox();
  expect(Math.round(after!.y)).toBe(Math.round(before!.y));
  expect(Math.round(after!.height)).toBe(Math.round(before!.height));
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
