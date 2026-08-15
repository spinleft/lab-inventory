import { describe, expect, it } from "vitest";
import { testGuestUser, testLabAdminUser, testRootUser } from "../shared/test/fixtures";
import { mobileTabItems } from "./MobileShell";
import { moduleNavItems } from "./modules";

function tabsFor(user: Parameters<NonNullable<(typeof moduleNavItems)[number]["canAccess"]>>[0]) {
  const visible = moduleNavItems.filter((item) => !item.canAccess || item.canAccess(user));
  return mobileTabItems(visible).map((item) => item.path);
}

describe("mobileTabItems", () => {
  it("leaves a slot for 更多 by never filling more than four", () => {
    for (const user of [testRootUser, testLabAdminUser, testGuestUser]) {
      expect(tabsFor(user).length).toBeLessThanOrEqual(4);
    }
  });

  it("puts the everyday screens up front for a laboratory admin", () => {
    expect(tabsFor(testLabAdminUser)).toEqual([
      "/dashboard",
      "/inventory",
      "/scan",
      "/borrow-requests",
    ]);
  });

  // A guest reaches neither the review queue nor the asset list, and would be
  // left with two tabs if the bar were a fixed list.
  it("falls back to what a guest can actually open", () => {
    const tabs = tabsFor(testGuestUser);
    expect(tabs).toContain("/dashboard");
    expect(tabs).toContain("/borrow-requests/mine");
    expect(tabs).not.toContain("/borrow-requests");
  });

  it("only offers paths the user is allowed to reach", () => {
    const guestPaths = new Set(
      moduleNavItems
        .filter((item) => !item.canAccess || item.canAccess(testGuestUser))
        .map((item) => item.path),
    );
    for (const path of tabsFor(testGuestUser)) {
      expect(guestPaths.has(path)).toBe(true);
    }
  });
});
