import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { testRegularUser, testRootUser } from "../shared/test/fixtures";
import { renderRoute, signIn } from "../shared/test/render";

async function openCommandMenu(user: ReturnType<typeof renderRoute>["user"]) {
  await user.click(await screen.findByRole("button", { name: "搜索" }));
  return within(await screen.findByRole("dialog"));
}

describe("CommandMenu", () => {
  it("opens from the sidebar search button", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    const menu = await openCommandMenu(user);

    expect(menu.getByPlaceholderText("搜索页面、操作或设置...")).toBeInTheDocument();
    expect(menu.getByText("打开概览")).toBeInTheDocument();
  });

  it("opens and closes with the ctrl+k shortcut", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    await user.keyboard("{Control>}k{/Control}");
    expect(await screen.findByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Control>}k{/Control}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("filters the command list as you type", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    const menu = await openCommandMenu(user);
    await user.type(menu.getByPlaceholderText("搜索页面、操作或设置..."), "审计");

    expect(menu.getByText("查看审计日志")).toBeInTheDocument();
    expect(menu.queryByText("打开概览")).not.toBeInTheDocument();
  });

  it("reports when nothing matches", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    const menu = await openCommandMenu(user);
    await user.type(
      menu.getByPlaceholderText("搜索页面、操作或设置..."),
      "zzzzz-no-such-command",
    );

    expect(menu.getByText("没有匹配的操作")).toBeInTheDocument();
  });

  it("navigates to the selected command and closes", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    const menu = await openCommandMenu(user);
    await user.click(menu.getByText("打开个人资料"));

    expect(await screen.findByRole("heading", { name: "个人资料" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("hides commands the current role cannot reach", async () => {
    signIn(testRegularUser);
    const { user } = renderRoute(["/dashboard"]);
    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();

    const menu = await openCommandMenu(user);

    expect(menu.getByText("查看资产")).toBeInTheDocument();
    expect(menu.queryByText("查看审计日志")).not.toBeInTheDocument();
    expect(menu.queryByText("管理用户")).not.toBeInTheDocument();
  });
});
