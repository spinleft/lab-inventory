import { screen } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../shared/api/backendConfig";
import { testCurrentUser } from "../shared/test/fixtures";
import { renderRoute } from "../shared/test/render";
import { server } from "../shared/test/server";

describe("RootRoute", () => {
  it("renders server settings when no backend URL has been configured", async () => {
    renderRoute(["/"]);

    expect(
      await screen.findByRole("heading", { name: "后端服务器设置" }),
    ).toBeInTheDocument();
  });

  it("redirects configured anonymous users to login", async () => {
    window.localStorage.setItem(
      BACKEND_CONFIG_STORAGE_KEY,
      "http://127.0.0.1:8000/api/v1",
    );
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
  });

  it("redirects configured authenticated users to the dashboard", async () => {
    window.localStorage.setItem(
      BACKEND_CONFIG_STORAGE_KEY,
      "http://127.0.0.1:8000/api/v1",
    );
    server.use(
      http.get("*/api/v1/auth/me", () => HttpResponse.json(testCurrentUser)),
    );

    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.getByText(testCurrentUser.username)).toBeInTheDocument();
  });

  it("shows a readable connection error when the session check fails", async () => {
    window.localStorage.setItem(
      BACKEND_CONFIG_STORAGE_KEY,
      "http://127.0.0.1:8000/api/v1",
    );
    server.use(http.get("*/api/v1/auth/me", () => HttpResponse.error()));

    renderRoute(["/"]);

    expect(await screen.findByText("无法连接后端")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "服务器设置" })).toBeInTheDocument();
  });

  it("always renders server settings at /server-settings", async () => {
    window.localStorage.setItem(
      BACKEND_CONFIG_STORAGE_KEY,
      "http://127.0.0.1:8000/api/v1",
    );

    renderRoute(["/server-settings"]);

    expect(
      await screen.findByRole("heading", { name: "后端服务器设置" }),
    ).toBeInTheDocument();
  });
});
