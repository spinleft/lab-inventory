import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import {
  testFederationTrust,
  testGuestUser,
  testLabAdminUser,
  testRegularUser,
  testRootUser,
} from "../shared/test/fixtures";
import { configureBackend, renderRoute, signIn } from "../shared/test/render";
import { server } from "../shared/test/server";

function sidebar() {
  const nav = screen.getByRole("navigation", { name: "主导航" });
  return within(nav);
}

async function openUserMenu(user: ReturnType<typeof renderRoute>["user"], username: string) {
  await user.click(screen.getByRole("button", { name: `用户菜单 ${username}` }));
}

describe("AppShell navigation", () => {
  it("shows workspace, admin and audit entries for root", async () => {
    signIn(testRootUser);
    renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    for (const label of [
      "概览",
      "资产",
      "库存",
      "实验室",
      "用户",
      "资产分类",
      "资产参数",
      "位置",
      "单位",
      "审计日志",
    ]) {
      expect(sidebar().getByRole("link", { name: label })).toBeInTheDocument();
    }
    // root has no laboratory, so borrowing and federation stay hidden.
    expect(sidebar().queryByRole("link", { name: "借用管理" })).not.toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "联邦实验室" })).not.toBeInTheDocument();
  });

  it("shows federation, borrowing and units for a laboratory admin but hides audit", async () => {
    signIn(testLabAdminUser);
    renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    expect(sidebar().getByRole("link", { name: "联邦实验室" })).toBeInTheDocument();
    expect(sidebar().getByRole("link", { name: "借用管理" })).toBeInTheDocument();
    // Units belong to the laboratory, so its admin maintains them.
    expect(sidebar().getByRole("link", { name: "单位" })).toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "审计日志" })).not.toBeInTheDocument();
  });

  it("hides laboratory and user administration from a regular user", async () => {
    signIn(testRegularUser);
    renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    expect(sidebar().queryByRole("link", { name: "实验室" })).not.toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "用户" })).not.toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "联邦实验室" })).not.toBeInTheDocument();
    // A plain user still curates the catalogue for their own laboratory.
    expect(sidebar().getByRole("link", { name: "资产分类" })).toBeInTheDocument();
    expect(sidebar().getByRole("link", { name: "位置" })).toBeInTheDocument();
  });

  it("leaves a guest with only the read-only workspace entries", async () => {
    signIn(testGuestUser);
    renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    expect(sidebar().getByRole("link", { name: "资产" })).toBeInTheDocument();
    expect(sidebar().getByRole("link", { name: "库存" })).toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "借用管理" })).not.toBeInTheDocument();
    expect(sidebar().queryByRole("link", { name: "资产分类" })).not.toBeInTheDocument();
  });

  it("names the current route in the breadcrumb and falls back for unknown routes", async () => {
    signIn(testRootUser);
    renderRoute(["/settings/password"]);

    expect(await screen.findByRole("heading", { name: "修改密码" })).toBeInTheDocument();
    expect(document.querySelector(".breadcrumb")).toHaveTextContent("修改密码");
  });
});

describe("AppShell user menu", () => {
  it("offers profile shortcuts and navigates to them", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await openUserMenu(user, "root");
    expect(await screen.findByRole("menuitem", { name: "个人资料" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "修改密码" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "偏好设置" })).toBeInTheDocument();

    await user.click(screen.getByRole("menuitem", { name: "个人资料" }));
    expect(await screen.findByRole("heading", { name: "个人资料" })).toBeInTheDocument();
  });

  it("logs out through the backend and returns to login", async () => {
    configureBackend();
    let signedIn = true;
    let logoutCalled = false;
    server.use(
      // The session has to survive until logout resolves, then disappear.
      http.get("*/api/v1/auth/me", () =>
        signedIn
          ? HttpResponse.json(testRootUser)
          : HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
      ),
      http.post("*/api/v1/auth/logout", () => {
        logoutCalled = true;
        signedIn = false;
        return HttpResponse.json({ message: "Logout successful" });
      }),
    );
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await openUserMenu(user, "root");
    await user.click(await screen.findByRole("menuitem", { name: "退出登录" }));

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(logoutCalled).toBe(true);
  });
});

describe("AppShell laboratory selector", () => {
  it("lists local laboratories for a system administrator", async () => {
    signIn(testRootUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(await screen.findByLabelText("选择实验室范围")).toBeInTheDocument();
  });

  it("offers an active federation trust alongside the local laboratory", async () => {
    signIn(testLabAdminUser);
    server.use(
      http.get("*/api/v1/local/federation/trusts", () =>
        HttpResponse.json([testFederationTrust]),
      ),
    );
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await user.click(await screen.findByLabelText("选择实验室范围"));
    expect(await screen.findByText(/远端实验室/)).toBeInTheDocument();
  });

  it("shows a static laboratory name when the user cannot switch scope", async () => {
    signIn(testGuestUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.queryByLabelText("选择实验室范围")).not.toBeInTheDocument();
    expect(document.querySelector(".sidebar-laboratory-static")).toHaveTextContent(
      "化学实验室",
    );
  });
});

describe("AppShell chrome", () => {
  it("switches the theme from the top bar", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "切换主题" }));
    await user.click(await screen.findByRole("menuitem", { name: "深色" }));

    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("opens the mobile navigation drawer", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "打开导航" }));

    expect(await screen.findByRole("dialog")).toHaveTextContent("导航");
  });
});
