import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  testLabAdminUser,
  testRegularUser,
  testRootUser,
} from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";

describe("DashboardPage", () => {
  it("summarises a global administrator and offers every entry", async () => {
    signIn(testRootUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    // Scope to the page: the sidebar repeats several of these labels.
    const page = within(screen.getByRole("main"));
    expect(page.getByText("全部")).toBeInTheDocument();
    expect(page.getByText("在线")).toBeInTheDocument();
    expect(page.getByRole("link", { name: /实验室/ })).toHaveAttribute(
      "href",
      "/admin/laboratories",
    );
    expect(page.getByRole("link", { name: /用户/ })).toHaveAttribute("href", "/admin/users");
    expect(page.getByRole("link", { name: /审计日志/ })).toHaveAttribute(
      "href",
      "/audit-logs",
    );
  });

  it("names the bound laboratory as the data scope", async () => {
    signIn(testLabAdminUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(within(screen.getByRole("main")).getByText("化学实验室")).toBeInTheDocument();
  });

  it("hides audit logs from a laboratory administrator", async () => {
    signIn(testLabAdminUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    const panelBody = document.querySelector(".panel-body") as HTMLElement;
    expect(panelBody.querySelector('a[href="/admin/laboratories"]')).not.toBeNull();
    expect(panelBody.querySelector('a[href="/audit-logs"]')).toBeNull();
  });

  it("offers no administrative entries to a regular user", async () => {
    signIn(testRegularUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    const panelBody = document.querySelector(".panel-body") as HTMLElement;
    expect(panelBody.querySelector('a[href="/admin/laboratories"]')).toBeNull();
    expect(panelBody.querySelector('a[href="/admin/users"]')).toBeNull();
    expect(panelBody.querySelector('a[href="/audit-logs"]')).toBeNull();
  });
});
