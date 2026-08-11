import { screen } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../../shared/api/backendConfig";
import { configureBackend, renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

const DEFAULT_URL = "http://127.0.0.1:8000/api/v1";

describe("ServerSettingsPage", () => {
  it("prefills the default backend address", async () => {
    renderRoute(["/server-settings"]);

    expect(await screen.findByLabelText(/后端 API 地址/)).toHaveValue(DEFAULT_URL);
  });

  it("rejects an empty address", async () => {
    const { user } = renderRoute(["/server-settings"]);

    await user.clear(await screen.findByLabelText(/后端 API 地址/));
    await user.click(screen.getByRole("button", { name: /保存并继续/ }));

    expect(await screen.findByText("请输入后端 API 地址。")).toBeInTheDocument();
  });

  it("rejects a malformed address", async () => {
    const { user } = renderRoute(["/server-settings"]);

    const input = await screen.findByLabelText(/后端 API 地址/);
    await user.clear(input);
    await user.type(input, "not-a-url");
    await user.click(screen.getByRole("button", { name: /保存并继续/ }));

    expect(await screen.findByText("后端 API 地址必须是有效的 URL。")).toBeInTheDocument();
  });

  it("rejects a non-http protocol", async () => {
    const { user } = renderRoute(["/server-settings"]);

    const input = await screen.findByLabelText(/后端 API 地址/);
    await user.clear(input);
    await user.type(input, "ftp://example.com");
    await user.click(screen.getByRole("button", { name: /保存并继续/ }));

    expect(
      await screen.findByText("后端 API 地址必须使用 http 或 https。"),
    ).toBeInTheDocument();
  });

  it("reports a failing health check without saving", async () => {
    server.use(
      http.get("*/api/v1/health_check", () => new HttpResponse(null, { status: 503 })),
    );
    const { user } = renderRoute(["/server-settings"]);

    await user.click(await screen.findByRole("button", { name: /保存并继续/ }));

    expect(await screen.findByText("健康检查失败：HTTP 503")).toBeInTheDocument();
    expect(window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY)).toBeNull();
  });

  it("saves a healthy backend and continues to login", async () => {
    const { user } = renderRoute(["/server-settings"]);

    await user.click(await screen.findByRole("button", { name: /保存并继续/ }));

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY)).toBe(DEFAULT_URL);
  });

  it("appends the api prefix to a bare origin", async () => {
    const { user } = renderRoute(["/server-settings"]);

    const input = await screen.findByLabelText(/后端 API 地址/);
    await user.clear(input);
    await user.type(input, "http://lab.example.com");
    await user.click(screen.getByRole("button", { name: /保存并继续/ }));

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY)).toBe(
      "http://lab.example.com/api/v1",
    );
  });

  it("resets a stored address back to the default", async () => {
    configureBackend("http://stored.example.com/api/v1");
    const { user } = renderRoute(["/server-settings"]);

    expect(await screen.findByLabelText(/后端 API 地址/)).toHaveValue(
      "http://stored.example.com/api/v1",
    );

    await user.click(screen.getByRole("button", { name: /重置/ }));

    expect(screen.getByLabelText(/后端 API 地址/)).toHaveValue(DEFAULT_URL);
    expect(window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY)).toBeNull();
  });
});
