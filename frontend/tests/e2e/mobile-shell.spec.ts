import { expect, test } from "@playwright/test";
import { isPhoneShell } from "./shell";

const apiBaseUrl = "http://127.0.0.1:8000/api/v1";
const currentUser = {
  email: null,
  laboratory: null,
  user_id: "00000000-0000-4000-8000-000000000001",
  user_type: {
    name: "root",
    user_type_id: "00000000-0000-4000-8000-000000000002",
  },
  username: "root",
};

/**
 * The phone shell is a fixed-height grid: title bar, scrolling content, tab
 * bar. The bug this guards against was a `min-height`, which let the content
 * row grow with its content and pushed the tab bar off the bottom of any page
 * taller than the screen.
 */
test("keeps the tab bar on screen however long the page is", async ({ page }) => {
  await page.route("**/api/v1/health_check", (route) =>
    route.fulfill({ status: 200, body: "OK" }),
  );
  await page.route("**/api/v1/auth/me", (route) => route.fulfill({ json: currentUser }));
  await page.route("**/api/v1/admin/laboratories", (route) => route.fulfill({ json: [] }));
  await page.addInitScript((url) => {
    window.localStorage.setItem("labInventory.apiBaseUrl", url);
  }, apiBaseUrl);

  await page.goto("/dashboard");
  await expect(page.getByRole("heading", { name: "概览" })).toBeVisible();

  test.skip(!(await isPhoneShell(page)), "只有手机外壳才有底部 tab 栏");

  // Short enough that the dashboard cannot fit — a landscape phone, or a
  // keyboard taking half the screen.
  await page.setViewportSize({ height: 320, width: 390 });

  const tabbar = page.locator(".tabbar");
  await expect(tabbar).toBeVisible();

  const viewportHeight = page.viewportSize()?.height ?? 0;
  const box = await tabbar.boundingBox();
  expect(box).not.toBeNull();
  // Fully inside the viewport, flush with its bottom edge.
  expect(Math.round(box!.y + box!.height)).toBeLessThanOrEqual(viewportHeight);
  expect(Math.round(box!.y + box!.height)).toBeGreaterThan(viewportHeight - 2);

  // The page itself must not scroll; the content row does.
  const documentScrolls = await page.evaluate(() => {
    const root = document.scrollingElement ?? document.documentElement;
    return root.scrollHeight > root.clientHeight + 1;
  });
  expect(documentScrolls).toBe(false);

  // And scrolling the content does not take the tab bar with it.
  await page.locator(".mobile-scroll").evaluate((node) => node.scrollTo(0, 10_000));
  const afterScroll = await tabbar.boundingBox();
  expect(Math.round(afterScroll!.y)).toBe(Math.round(box!.y));
});
