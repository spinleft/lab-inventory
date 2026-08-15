import { expect, type Page } from "@playwright/test";

/**
 * Shell-aware helpers.
 *
 * The same suite runs against three viewports, and below 768px the app is a
 * different shell: bottom tabs instead of a sidebar, with appearance and sign
 * out living on the "更多" tab rather than in the sidebar's user menu. Tests
 * that only care about the outcome go through these.
 */

export function isPhoneShell(page: Page) {
  return page.locator(".tabbar").isVisible();
}

/** Opens the phone shell's overflow tab. No-op on the desktop shell. */
async function openMoreTab(page: Page) {
  const moreTab = page.getByRole("link", { name: "更多" });
  await moreTab.click();
  await expect(page.getByRole("heading", { name: "外观" })).toBeVisible();
}

export async function setTheme(page: Page, label: string) {
  if (await isPhoneShell(page)) {
    await openMoreTab(page);
    await page.getByRole("button", { name: label }).click();
    return;
  }

  await page.getByRole("button", { name: "切换主题" }).click();
  await page.getByText(label).click();
}

export async function signOut(page: Page) {
  if (await isPhoneShell(page)) {
    await openMoreTab(page);
    await page.getByRole("button", { name: "退出登录" }).click();
    return;
  }

  const userMenuButton = page.getByRole("button", { name: /用户菜单 / });
  if (!(await userMenuButton.isVisible().catch(() => false))) {
    // The tablet shell keeps the sidebar behind a drawer.
    await page.getByRole("button", { name: "打开导航" }).click();
    await expect(userMenuButton).toBeVisible();
  }
  await userMenuButton.click();
  await page.getByText("退出登录").click();
}

/** Follows a navigation entry, wherever this shell keeps it. */
export async function openNavItem(page: Page, name: string) {
  const link = page.getByRole("link", { exact: true, name });
  if (await link.first().isVisible().catch(() => false)) {
    await link.first().click();
    return;
  }

  if (await isPhoneShell(page)) {
    await openMoreTab(page);
  } else {
    await page.getByRole("button", { name: "打开导航" }).click();
  }
  await link.first().click();
}
