import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../../shared/api/backendConfig";
import { testCurrentUser } from "../../shared/test/fixtures";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("ServerSettingsPage", () => {
  it("renders the root server settings page", async () => {
    renderRoute(["/server-settings"]);

    expect(
      await screen.findByRole("heading", { name: "后端服务器设置" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("后端 API 地址")).toBeInTheDocument();
  });

  it("normalizes a server root URL and redirects anonymous users to login", async () => {
    const user = userEvent.setup();
    renderRoute(["/server-settings"]);

    const apiInput = screen.getByLabelText("后端 API 地址");
    await user.clear(apiInput);
    await user.type(apiInput, "http://127.0.0.1:8000");
    await user.click(screen.getByRole("button", { name: "继续" }));

    expect(
      await screen.findByRole("heading", { name: /^登录$/ }),
    ).toBeInTheDocument();
    expect(window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY)).toBe(
      "http://127.0.0.1:8000/api/v1",
    );
  });

  it("redirects authenticated users to the dashboard when continuing", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/api/v1/auth/me", () => HttpResponse.json(testCurrentUser)),
    );
    renderRoute(["/server-settings"]);

    await user.click(screen.getByRole("button", { name: "继续" }));

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.getByText(testCurrentUser.username)).toBeInTheDocument();
  });

  it("tests the backend health endpoint", async () => {
    const user = userEvent.setup();
    renderRoute(["/server-settings"]);

    await user.click(screen.getByRole("button", { name: "测试连接" }));

    expect(await screen.findByText("连接正常。")).toBeInTheDocument();
  });
});
