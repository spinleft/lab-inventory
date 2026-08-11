import { screen } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { testRootUser } from "../../shared/test/fixtures";
import { configureBackend, renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("LoginPage", () => {
  it("redirects to server settings when no backend is configured", async () => {
    renderRoute(["/login"]);

    expect(await screen.findByRole("heading", { name: "服务端" })).toBeInTheDocument();
  });

  it("redirects an already authenticated user to the dashboard", async () => {
    signIn(testRootUser);
    renderRoute(["/login"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });

  it("keeps submit disabled until both fields are filled", async () => {
    configureBackend();
    const { user } = renderRoute(["/login"]);

    const submit = await screen.findByRole("button", { name: /登录/ });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("用户名"), "root");
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("密码"), "password");
    expect(submit).toBeEnabled();
  });

  it("ignores a username that is only whitespace", async () => {
    configureBackend();
    const { user } = renderRoute(["/login"]);

    await user.type(await screen.findByLabelText("用户名"), "   ");
    await user.type(screen.getByLabelText("密码"), "password");

    expect(screen.getByRole("button", { name: /登录/ })).toBeDisabled();
  });

  it("signs in and lands on the dashboard", async () => {
    configureBackend();
    let signedIn = false;
    let submitted: unknown;
    server.use(
      http.get("*/api/v1/auth/me", () =>
        signedIn
          ? HttpResponse.json(testRootUser)
          : HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
      ),
      http.post("*/api/v1/auth/login", async ({ request }) => {
        submitted = await request.json();
        signedIn = true;
        return HttpResponse.json({ message: "Login successful" });
      }),
    );
    const { user } = renderRoute(["/login"]);

    await user.type(await screen.findByLabelText("用户名"), "root");
    await user.type(screen.getByLabelText("密码"), "password");
    await user.click(screen.getByRole("button", { name: /登录/ }));

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(submitted).toEqual({ password: "password", username: "root" });
  });

  it("surfaces the backend message when sign in fails", async () => {
    configureBackend();
    server.use(
      http.post("*/api/v1/auth/login", () =>
        HttpResponse.json({ error: "用户名或密码错误" }, { status: 401 }),
      ),
    );
    const { user } = renderRoute(["/login"]);

    await user.type(await screen.findByLabelText("用户名"), "root");
    await user.type(screen.getByLabelText("密码"), "wrong");
    await user.click(screen.getByRole("button", { name: /登录/ }));

    expect(await screen.findByText("登录失败")).toBeInTheDocument();
    expect(await screen.findByText("用户名或密码错误")).toBeInTheDocument();
    // Still on the login page.
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
  });

  it("links back to server settings", async () => {
    configureBackend();
    renderRoute(["/login"]);

    expect(await screen.findByRole("link", { name: /服务端/ })).toHaveAttribute(
      "href",
      "/server-settings",
    );
  });
});
