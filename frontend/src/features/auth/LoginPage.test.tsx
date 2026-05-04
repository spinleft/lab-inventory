import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("LoginPage", () => {
  it("validates required credentials", async () => {
    const user = userEvent.setup();
    renderRoute(["/login"]);

    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(screen.getByText("请输入用户名和密码。")).toBeInTheDocument();
  });

  it("shows backend authentication errors", async () => {
    const user = userEvent.setup();
    server.use(
      http.post("*/api/v1/auth/login", () =>
        HttpResponse.json({ error: "Authentication failed" }, { status: 401 }),
      ),
    );
    renderRoute(["/login"]);

    await user.type(screen.getByLabelText("用户名"), "wrong");
    await user.type(screen.getByLabelText("密码"), "wrong");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("Authentication failed")).toBeInTheDocument();
  });

  it("allows the backend server to be changed before login", async () => {
    const user = userEvent.setup();
    renderRoute(["/login"]);

    await user.click(screen.getByRole("button", { name: /服务器设置/ }));
    const apiInput = screen.getByLabelText("后端 API 地址");
    await user.clear(apiInput);
    await user.type(apiInput, "http://backend.example.test:8080");
    await user.click(screen.getByRole("button", { name: "测试连接" }));

    expect(await screen.findByText("连接正常。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(
      screen.getByText("http://backend.example.test:8080/api/v1"),
    ).toBeInTheDocument();
  });

  it("enters the protected shell after a successful login", async () => {
    const user = userEvent.setup();
    renderRoute(["/login"]);

    await user.type(screen.getByLabelText("用户名"), "admin");
    await user.type(screen.getByLabelText("密码"), "password");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });
});
