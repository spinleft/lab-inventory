import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { testGuestUser, testRegularUser, testRootUser } from "../shared/test/fixtures";
import { renderRoute, signIn } from "../shared/test/render";

describe("ProtectedModuleRoute", () => {
  it("blocks a route the current role cannot reach", async () => {
    signIn(testRegularUser);
    renderRoute(["/admin/users"]);

    expect(await screen.findByRole("heading", { name: "无法访问" })).toBeInTheDocument();
    expect(screen.getByText("权限不足")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "返回概览" })).toHaveAttribute(
      "href",
      "/dashboard",
    );
  });

  it("blocks audit logs for a guest", async () => {
    signIn(testGuestUser);
    renderRoute(["/audit-logs"]);

    expect(await screen.findByRole("heading", { name: "无法访问" })).toBeInTheDocument();
  });

  it("renders the page when the role is allowed", async () => {
    signIn(testRootUser);
    renderRoute(["/admin/users"]);

    expect(await screen.findByRole("heading", { name: "用户" })).toBeInTheDocument();
    expect(screen.queryByText("权限不足")).not.toBeInTheDocument();
  });

  it("redirects unknown paths back to the root route", async () => {
    signIn(testRootUser);
    renderRoute(["/does-not-exist"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
  });
});
