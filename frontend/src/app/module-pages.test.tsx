import { screen } from "@testing-library/react";
import { delay, http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import {
  testGuestUser,
  testLabAdminUser,
  testRootUser,
} from "../shared/test/fixtures";
import { renderRoute, signIn } from "../shared/test/render";
import { server } from "../shared/test/server";
import { type CurrentUser } from "../modules/auth/types";

/**
 * Every routed module page, with the role that is allowed to open it. These
 * cover the render path only — the behaviour of each page's filters and
 * dialogs belongs in that module's own test file.
 */
const PAGES: Array<{ as: CurrentUser; heading: string; path: string }> = [
  { as: testRootUser, heading: "资产", path: "/assets" },
  { as: testRootUser, heading: "库存", path: "/inventory" },
  { as: testRootUser, heading: "实验室", path: "/admin/laboratories" },
  { as: testRootUser, heading: "用户", path: "/admin/users" },
  { as: testRootUser, heading: "资产分类", path: "/admin/asset-categories" },
  { as: testRootUser, heading: "资产参数", path: "/admin/asset-parameters" },
  { as: testRootUser, heading: "位置", path: "/admin/locations" },
  { as: testRootUser, heading: "单位", path: "/admin/units" },
  // Units are laboratory data: a laboratory admin opens the same page through
  // /local rather than /admin/laboratories/{id}, which is a different request
  // path and so worth rendering on its own.
  { as: testLabAdminUser, heading: "单位", path: "/admin/units" },
  { as: testRootUser, heading: "审计日志", path: "/audit-logs" },
  { as: testLabAdminUser, heading: "联邦实验室", path: "/admin/federation" },
  { as: testLabAdminUser, heading: "借用管理", path: "/borrow-requests" },
  // A guest reaches no other borrow surface, so this page is checked as one.
  { as: testGuestUser, heading: "我的借用", path: "/borrow-requests/mine" },
  { as: testRootUser, heading: "资产详情", path: "/assets/00000000-0000-4000-8000-0000000000c1" },
  {
    as: testRootUser,
    heading: "库存详情",
    path: "/inventory/00000000-0000-4000-8000-0000000000c2",
  },
];

describe("module pages render", () => {
  for (const { as, heading, path } of PAGES) {
    it(`renders ${path}`, async () => {
      signIn(as);
      renderRoute([path]);

      expect(
        await screen.findByRole("heading", { level: 1, name: heading }),
      ).toBeInTheDocument();
    });
  }
});

describe("module pages tolerate slow and failing backends", () => {
  const DATA_PAGES = PAGES.filter(
    (page) => !page.path.includes("/assets/0") && !page.path.includes("/inventory/0"),
  );

  for (const { as, heading, path } of DATA_PAGES) {
    it(`keeps ${path} usable while its data loads`, async () => {
      signIn(as);
      // Every collection request hangs; the page must still paint its header.
      server.use(
        http.get("*/api/v1/admin/laboratories/:laboratoryId/:collection", async () => {
          await delay("infinite");
          return HttpResponse.json([]);
        }),
        http.get("*/api/v1/local/:collection", async () => {
          await delay("infinite");
          return HttpResponse.json([]);
        }),
        http.get("*/api/v1/local/federation/:collection", async () => {
          await delay("infinite");
          return HttpResponse.json([]);
        }),
        http.get("*/api/v1/audit-logs", async () => {
          await delay("infinite");
          return HttpResponse.json({ items: [], limit: 20, offset: 0, total: 0 });
        }),
      );
      renderRoute([path]);

      expect(
        await screen.findByRole("heading", { level: 1, name: heading }),
      ).toBeInTheDocument();
    });

    it(`keeps ${path} usable when its data fails`, async () => {
      signIn(as);
      server.use(
        http.get("*/api/v1/admin/laboratories/:laboratoryId/:collection", () =>
          HttpResponse.json({ error: "后端故障" }, { status: 500 }),
        ),
        http.get("*/api/v1/local/:collection", () =>
          HttpResponse.json({ error: "后端故障" }, { status: 500 }),
        ),
        http.get("*/api/v1/local/federation/:collection", () =>
          HttpResponse.json({ error: "后端故障" }, { status: 500 }),
        ),
        http.get("*/api/v1/audit-logs", () =>
          HttpResponse.json({ error: "后端故障" }, { status: 500 }),
        ),
      );
      renderRoute([path]);

      expect(
        await screen.findByRole("heading", { level: 1, name: heading }),
      ).toBeInTheDocument();
    });
  }
});
