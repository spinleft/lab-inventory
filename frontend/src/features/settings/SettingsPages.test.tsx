import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../../shared/api/backendConfig";
import {
  testCurrentUser,
  testMaintainerUser,
  testRegularUser,
} from "../../shared/test/fixtures";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("SettingsPages", () => {
  it("shows profile information", async () => {
    renderAuthenticatedRoute("/settings/profile", testCurrentUser);

    expect(await screen.findByText("账号信息")).toBeInTheDocument();
    expect(screen.getByText("admin@example.com")).toBeInTheDocument();
    expect(screen.getByText("系统所有者")).toBeInTheDocument();
  });

  it("changes the current user's password", async () => {
    const user = userEvent.setup();
    renderAuthenticatedRoute("/settings/password", testCurrentUser);

    await user.type(await screen.findByLabelText("当前密码"), "old-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "new-password");
    await user.click(screen.getByRole("button", { name: "保存密码" }));

    expect(await screen.findByText("密码已更新。")).toBeInTheDocument();
  });

  it("shows password change backend errors", async () => {
    const user = userEvent.setup();
    server.use(
      http.patch("*/api/v1/auth/password", () =>
        HttpResponse.json(
          { error: "Current password is incorrect" },
          { status: 401 },
        ),
      ),
    );
    renderAuthenticatedRoute("/settings/password", testCurrentUser);

    await user.type(await screen.findByLabelText("当前密码"), "wrong-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "new-password");
    await user.click(screen.getByRole("button", { name: "保存密码" }));

    expect(await screen.findByText("Current password is incorrect")).toBeInTheDocument();
  });

  it("shows password confirmation mismatch errors", async () => {
    const user = userEvent.setup();
    server.use(
      http.patch("*/api/v1/auth/password", () =>
        HttpResponse.json(
          { error: "New password confirmation does not match" },
          { status: 400 },
        ),
      ),
    );
    renderAuthenticatedRoute("/settings/password", testCurrentUser);

    await user.type(await screen.findByLabelText("当前密码"), "old-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "different-password");
    await user.click(screen.getByRole("button", { name: "保存密码" }));

    expect(
      await screen.findByText("New password confirmation does not match"),
    ).toBeInTheDocument();
  });

  it("renders preference settings route", async () => {
    renderAuthenticatedRoute("/settings/preference", testRegularUser);
    expect(
      await screen.findByRole("heading", { level: 1, name: "偏好设置" }),
    ).toBeInTheDocument();
    expect(screen.getByText("当前仅调整路由和页面框架，不新增配置项。")).toBeInTheDocument();
  });

  it("redirects old settings routes to the new settings routes", async () => {
    renderAuthenticatedRoute("/settings/system", testCurrentUser);
    expect(
      await screen.findByRole("heading", { level: 1, name: "偏好设置" }),
    ).toBeInTheDocument();
  });

  it("allows admin center for maintainers with laboratory scope messaging", async () => {
    renderAuthenticatedRoute("/admin", testMaintainerUser);
    expect((await screen.findAllByText("管理中心")).length).toBeGreaterThan(0);
    expect(screen.getByText("你只能管理自己实验室范围内的数据。")).toBeInTheDocument();
  });

  it("renders admin resource routes as placeholders", async () => {
    renderAuthenticatedRoute("/admin/laboratories", testCurrentUser);
    expect(await screen.findByRole("heading", { level: 1, name: "实验室" })).toBeInTheDocument();
    expect(screen.getByText("实验室管理入口已经接入新的管理导航。实验室 CRUD 将在后续切片实现。")).toBeInTheDocument();
  });

  it("blocks admin center for regular users", async () => {
    renderAuthenticatedRoute("/admin", testRegularUser);
    expect(await screen.findByText("无权限访问")).toBeInTheDocument();
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
