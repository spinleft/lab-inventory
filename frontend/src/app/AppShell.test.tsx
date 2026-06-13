import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../shared/api/backendConfig";
import {
  testCurrentUser,
  testMaintainerUser,
  testRegularUser,
} from "../shared/test/fixtures";
import { renderRoute } from "../shared/test/render";
import { server } from "../shared/test/server";

describe("AppShell", () => {
  it("renders the dashboard shell when visiting dashboard directly", async () => {
    renderAuthenticatedRoute("/dashboard", testCurrentUser);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /admin/ })).toBeInTheDocument();
    expect(screen.getByText("后台导航")).toBeInTheDocument();
    const sideNavigation = screen.getByText("后台导航").closest(".app-sider-section");
    expect(sideNavigation).not.toBeNull();
    expect(within(sideNavigation as HTMLElement).getByText("概览")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("库存")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("借用")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("维护")).toBeInTheDocument();
    expect(
      within(sideNavigation as HTMLElement).getByText("审计日志"),
    ).toBeInTheDocument();
    expect(
      within(sideNavigation as HTMLElement).getByText("库存").closest("li"),
    ).toHaveAttribute("aria-disabled", "true");
    expect(
      within(sideNavigation as HTMLElement).queryByText("用户设置"),
    ).not.toBeInTheDocument();
    expect(
      within(sideNavigation as HTMLElement).queryByText("系统设置"),
    ).not.toBeInTheDocument();
    expect(
      within(sideNavigation as HTMLElement).queryByText("管理中心"),
    ).not.toBeInTheDocument();
  });

  it("renders settings navigation when visiting user settings", async () => {
    renderAuthenticatedRoute("/settings/profile", testCurrentUser);

    expect(await screen.findByRole("heading", { name: "用户资料" })).toBeInTheDocument();
    expect(screen.getByText("设置导航")).toBeInTheDocument();
    const sideNavigation = screen.getByText("设置导航").closest(".app-sider-section");
    expect(sideNavigation).not.toBeNull();
    expect(within(sideNavigation as HTMLElement).getByText("用户资料")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("密码")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("偏好设置")).toBeInTheDocument();
    expect(
      within(sideNavigation as HTMLElement).queryByText("管理中心"),
    ).not.toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).queryByText("库存")).not.toBeInTheDocument();
  });

  it("renders admin navigation for admins", async () => {
    renderAuthenticatedRoute("/admin", testMaintainerUser);

    expect(
      await screen.findByRole("heading", { level: 1, name: "管理中心" }),
    ).toBeInTheDocument();
    const sideNavigation = screen.getByText("管理导航").closest(".app-sider-section");
    expect(sideNavigation).not.toBeNull();
    expect(within(sideNavigation as HTMLElement).getByText("实验室")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("用户")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).getByText("远端实验室")).toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).queryByText("用户资料")).not.toBeInTheDocument();
  });

  it("keeps admin navigation empty for regular users", async () => {
    renderAuthenticatedRoute("/admin", testRegularUser);

    expect(await screen.findByText("无权限访问")).toBeInTheDocument();
    const sideNavigation = screen.getByText("管理导航").closest(".app-sider-section");
    expect(sideNavigation).not.toBeNull();
    expect(within(sideNavigation as HTMLElement).queryByText("实验室")).not.toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).queryByText("用户")).not.toBeInTheDocument();
    expect(within(sideNavigation as HTMLElement).queryByText("远端实验室")).not.toBeInTheDocument();
  });

  it("shows owner settings entries in the user menu", async () => {
    const user = userEvent.setup();
    renderAuthenticatedRoute("/dashboard", testCurrentUser);

    await user.click(await screen.findByRole("button", { name: /admin/ }));

    expect((await screen.findAllByText("用户设置")).length).toBeGreaterThan(0);
    expect(screen.queryByText("系统设置")).not.toBeInTheDocument();
    expect(screen.getAllByText("管理中心").length).toBeGreaterThan(0);
    expect(screen.getByText("登出")).toBeInTheDocument();
  });

  it("shows admin center for admins", async () => {
    const user = userEvent.setup();
    renderAuthenticatedRoute("/dashboard", testMaintainerUser);

    await user.click(await screen.findByRole("button", { name: /admin-user/ }));
    expect((await screen.findAllByText("用户设置")).length).toBeGreaterThan(0);
    expect(screen.queryByText("系统设置")).not.toBeInTheDocument();
    expect(screen.getAllByText("管理中心").length).toBeGreaterThan(0);
  });

  it("shows only user settings for regular users", async () => {
    const user = userEvent.setup();
    renderAuthenticatedRoute("/dashboard", testRegularUser);

    await user.click(await screen.findByRole("button", { name: /lab-user/ }));
    expect((await screen.findAllByText("用户设置")).length).toBeGreaterThan(0);
    expect(screen.queryByText("管理中心")).not.toBeInTheDocument();
  });

  it("logs out through the backend and redirects to login", async () => {
    const user = userEvent.setup();
    let logoutCalled = false;
    server.use(
      http.post("*/api/v1/auth/logout", () => {
        logoutCalled = true;
        return HttpResponse.json({ message: "Logout successful" });
      }),
    );
    renderAuthenticatedRoute("/dashboard", testCurrentUser);

    await user.click(await screen.findByRole("button", { name: /admin/ }));
    await user.click(await screen.findByText("登出"));

    expect(await screen.findByRole("heading", { name: /^登录$/ })).toBeInTheDocument();
    expect(logoutCalled).toBe(true);
  });
});

type TestUser =
  | typeof testCurrentUser
  | typeof testMaintainerUser
  | typeof testRegularUser;

function renderAuthenticatedRoute(route: string, currentUser: TestUser) {
  window.localStorage.setItem(
    BACKEND_CONFIG_STORAGE_KEY,
    "http://127.0.0.1:8000/api/v1",
  );
  server.use(http.get("*/api/v1/auth/me", () => HttpResponse.json(currentUser)));
  renderRoute([route]);
}
