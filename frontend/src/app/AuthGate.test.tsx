import { screen } from "@testing-library/react";
import { delay, http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { testRootUser } from "../shared/test/fixtures";
import { configureBackend, renderRoute, signIn } from "../shared/test/render";
import { server } from "../shared/test/server";

function pendingCurrentUser() {
  server.use(
    http.get("*/api/v1/auth/me", async () => {
      await delay("infinite");
      return HttpResponse.json(testRootUser);
    }),
  );
}

describe("RootRoute", () => {
  it("sends an unconfigured client to server settings", async () => {
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "服务端" })).toBeInTheDocument();
  });

  it("shows a waiting state while the session is being checked", async () => {
    configureBackend();
    pendingCurrentUser();
    renderRoute(["/"]);

    expect(await screen.findByText("准备工作台")).toBeInTheDocument();
  });

  it("redirects to login when the session is rejected", async () => {
    configureBackend();
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
  });

  it("redirects to server settings when the backend is unreachable", async () => {
    configureBackend();
    server.use(
      http.get("*/api/v1/auth/me", () =>
        HttpResponse.json({ error: "boom" }, { status: 500 }),
      ),
    );
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "服务端" })).toBeInTheDocument();
  });

  it("sends an authenticated user to the dashboard", async () => {
    signIn(testRootUser);
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });
});

describe("RequireAuth", () => {
  it("sends an unconfigured client to server settings", async () => {
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "服务端" })).toBeInTheDocument();
  });

  it("shows a waiting state while the session is being checked", async () => {
    configureBackend();
    pendingCurrentUser();
    renderRoute(["/dashboard"]);

    expect(await screen.findByText("检查登录状态")).toBeInTheDocument();
  });

  it("redirects to login when the session is rejected", async () => {
    configureBackend();
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
  });

  it("surfaces a backend error with a link back to server settings", async () => {
    configureBackend();
    server.use(
      http.get("*/api/v1/auth/me", () =>
        HttpResponse.json({ error: "连接被拒绝" }, { status: 503 }),
      ),
    );
    renderRoute(["/dashboard"]);

    expect(await screen.findByText("后端连接异常")).toBeInTheDocument();
    expect(screen.getByText("连接被拒绝")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /服务端设置/ })).toHaveAttribute(
      "href",
      "/server-settings",
    );
  });

  it("renders the protected page for an authenticated user", async () => {
    signIn(testRootUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });
});
