import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("ServerSettingsPage", () => {
  it("is the frontend root route", async () => {
    renderRoute(["/"]);

    expect(
      await screen.findByRole("heading", { name: "后端服务器设置" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("后端 API 地址")).toBeInTheDocument();
  });

  it("continues to login when the configured backend has no active session", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/api/v1/auth/me", () =>
        HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
      ),
    );
    renderRoute(["/"]);

    await user.click(await screen.findByRole("button", { name: "继续" }));

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
  });

  it("continues to the dashboard when the configured backend session is active", async () => {
    const user = userEvent.setup();
    renderRoute(["/"]);

    await user.click(await screen.findByRole("button", { name: "继续" }));

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });

  it("shows a readable connection error instead of the browser fetch message", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/api/v1/auth/me", () => HttpResponse.error()),
    );
    renderRoute(["/"]);

    await user.click(await screen.findByRole("button", { name: "继续" }));

    expect(
      await screen.findByText("无法连接后端，请确认地址、网络、CORS 和后端服务状态。"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Load failed")).not.toBeInTheDocument();
  });
});
