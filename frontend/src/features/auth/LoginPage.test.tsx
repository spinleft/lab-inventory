import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../../shared/api/backendConfig";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";
import { testCurrentUser } from "../../shared/test/fixtures";

describe("LoginPage", () => {
  it("validates required username and password fields", async () => {
    const user = userEvent.setup();
    renderRoute(["/login"]);

    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("请输入用户名。")).toBeInTheDocument();
    expect(screen.getByText("请输入密码。")).toBeInTheDocument();
  });

  it("submits credentials and redirects to dashboard on success", async () => {
    const user = userEvent.setup();
    let requestBody: unknown;
    window.localStorage.setItem(
      BACKEND_CONFIG_STORAGE_KEY,
      "http://127.0.0.1:8000/api/v1",
    );
    server.use(
      http.post("*/api/v1/auth/login", async ({ request }) => {
        requestBody = await request.json();
        return HttpResponse.json({ message: "Login successful" });
      }),
      http.get("*/api/v1/auth/me", () => HttpResponse.json(testCurrentUser)),
    );

    renderRoute(["/login"]);

    await user.type(screen.getByLabelText("用户名"), "root");
    await user.type(screen.getByLabelText("密码"), "password");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() =>
      expect(requestBody).toEqual({
        username: "root",
        password: "password",
      }),
    );
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });

  it("shows backend errors when login fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.post("*/api/v1/auth/login", () =>
        HttpResponse.json({ error: "Authentication failed" }, { status: 401 }),
      ),
    );

    renderRoute(["/login"]);

    await user.type(screen.getByLabelText("用户名"), "root");
    await user.type(screen.getByLabelText("密码"), "wrong-password");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("Authentication failed")).toBeInTheDocument();
  });
});
