import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { testGuestUser, testRegularUser, testRootUser } from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

async function fillPasswordForm(
  user: ReturnType<typeof renderRoute>["user"],
  values = { check: "new-password", current: "old-password", next: "new-password" },
) {
  await user.type(await screen.findByLabelText("当前密码"), values.current);
  await user.type(screen.getByLabelText("新密码"), values.next);
  await user.type(screen.getByLabelText("确认新密码"), values.check);
}

describe("ProfilePage", () => {
  it("describes a global administrator", async () => {
    signIn(testRootUser);
    renderRoute(["/settings/profile"]);

    expect(await screen.findByRole("heading", { name: "个人资料" })).toBeInTheDocument();
    // Scope to the page: the sidebar user button also renders the username.
    const page = within(screen.getByRole("main"));
    expect(page.getByText("root")).toBeInTheDocument();
    expect(page.getByText("root@example.com")).toBeInTheDocument();
    expect(page.getByText("系统根用户")).toBeInTheDocument();
    expect(page.getByText("全部实验室")).toBeInTheDocument();
  });

  it("describes a laboratory-scoped user", async () => {
    signIn(testRegularUser);
    renderRoute(["/settings/profile"]);

    expect(await screen.findByRole("heading", { name: "个人资料" })).toBeInTheDocument();
    const page = within(screen.getByRole("main"));
    expect(page.getByText("普通用户")).toBeInTheDocument();
    expect(page.getByText("化学实验室")).toBeInTheDocument();
  });

  it("falls back to a placeholder when the account has no email", async () => {
    signIn({ ...testGuestUser, email: null });
    renderRoute(["/settings/profile"]);

    expect(await screen.findByRole("heading", { name: "个人资料" })).toBeInTheDocument();
    const page = within(screen.getByRole("main"));
    expect(page.getByText("未设置")).toBeInTheDocument();
    expect(page.getByText("访客")).toBeInTheDocument();
  });
});

describe("PasswordPage", () => {
  it("keeps submit disabled until every field is filled", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/settings/password"]);

    const submit = await screen.findByRole("button", { name: /保存密码/ });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("当前密码"), "old-password");
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("新密码"), "new-password");
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("确认新密码"), "new-password");
    expect(submit).toBeEnabled();
  });

  it("submits the change and clears the form", async () => {
    signIn(testRootUser);
    let submitted: unknown;
    server.use(
      http.patch("*/api/v1/auth/password", async ({ request }) => {
        submitted = await request.json();
        return HttpResponse.json({ message: "Password changed" });
      }),
    );
    const { user } = renderRoute(["/settings/password"]);

    await fillPasswordForm(user);
    await user.click(screen.getByRole("button", { name: /保存密码/ }));

    expect(await screen.findByText("密码已更新")).toBeInTheDocument();
    expect(submitted).toEqual({
      current_password: "old-password",
      new_password: "new-password",
      new_password_check: "new-password",
    });
    expect(screen.getByLabelText("当前密码")).toHaveValue("");
    expect(screen.getByLabelText("新密码")).toHaveValue("");
    expect(screen.getByLabelText("确认新密码")).toHaveValue("");
  });

  it("surfaces the backend message and keeps the input when it fails", async () => {
    signIn(testRootUser);
    server.use(
      http.patch("*/api/v1/auth/password", () =>
        HttpResponse.json({ error: "当前密码不正确" }, { status: 400 }),
      ),
    );
    const { user } = renderRoute(["/settings/password"]);

    await fillPasswordForm(user);
    await user.click(screen.getByRole("button", { name: /保存密码/ }));

    expect(await screen.findByText("密码修改失败")).toBeInTheDocument();
    expect(await screen.findByText("当前密码不正确")).toBeInTheDocument();
    expect(screen.getByLabelText("当前密码")).toHaveValue("old-password");
  });
});

describe("PreferencesPage", () => {
  it("switches the theme and marks the active option", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/settings/preferences"]);

    expect(await screen.findByRole("heading", { name: "偏好设置" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "深色" }));
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("button", { name: "浅色" }));
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("restores the stored preference on load", async () => {
    window.localStorage.setItem("labInventory.theme", "dark");
    signIn(testRootUser);
    renderRoute(["/settings/preferences"]);

    expect(await screen.findByRole("heading", { name: "偏好设置" })).toBeInTheDocument();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
