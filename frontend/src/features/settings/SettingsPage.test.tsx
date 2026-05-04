import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

describe("SettingsPage", () => {
  it("opens from the protected shell toolbar", async () => {
    const user = userEvent.setup();
    renderRoute(["/"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "用户设置" }));

    expect(await screen.findByRole("heading", { name: "用户设置" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "密码" })).toBeInTheDocument();
  });

  it("validates password form fields before submitting", async () => {
    const user = userEvent.setup();
    renderRoute(["/settings/password"]);

    expect(await screen.findByRole("heading", { name: "用户设置" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "更新密码" }));
    expect(screen.getByText("请输入当前密码、新密码和确认密码。")).toBeInTheDocument();

    await user.type(screen.getByLabelText("当前密码"), "old-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "different-password");
    await user.click(screen.getByRole("button", { name: "更新密码" }));

    expect(screen.getByText("两次输入的新密码不一致。")).toBeInTheDocument();
  });

  it("submits password changes for the current user", async () => {
    const user = userEvent.setup();
    let payload: unknown;
    server.use(
      http.patch("*/api/v1/auth/password", async ({ request }) => {
        payload = await request.json();
        return HttpResponse.json({ message: "Password changed" });
      }),
    );
    renderRoute(["/settings/password"]);

    expect(await screen.findByRole("heading", { name: "用户设置" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("当前密码"), "old-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "new-password");
    await user.click(screen.getByRole("button", { name: "更新密码" }));

    expect(await screen.findByText("密码已更新。")).toBeInTheDocument();
    expect(payload).toEqual({
      current_password: "old-password",
      new_password: "new-password",
      new_password_check: "new-password",
    });
    expect(screen.getByLabelText("当前密码")).toHaveValue("");
    expect(screen.getByLabelText("新密码")).toHaveValue("");
    expect(screen.getByLabelText("确认新密码")).toHaveValue("");
  });

  it("shows backend password errors", async () => {
    const user = userEvent.setup();
    server.use(
      http.patch("*/api/v1/auth/password", () =>
        HttpResponse.json({ error: "Current password is incorrect" }, { status: 401 }),
      ),
    );
    renderRoute(["/settings/password"]);

    expect(await screen.findByRole("heading", { name: "用户设置" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("当前密码"), "wrong-password");
    await user.type(screen.getByLabelText("新密码"), "new-password");
    await user.type(screen.getByLabelText("确认新密码"), "new-password");
    await user.click(screen.getByRole("button", { name: "更新密码" }));

    expect(await screen.findByText("Current password is incorrect")).toBeInTheDocument();
  });
});
