import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppChrome } from "./AppChrome";

describe("AppChrome", () => {
  it("renders app chrome and forwards navigation actions", async () => {
    const user = userEvent.setup();
    const onBrandClick = vi.fn();
    const onSidebarSelect = vi.fn();
    const onUserMenuClick = vi.fn();

    render(
      <AppChrome
        breadcrumbItems={[
          { key: "dashboard", label: "后台", onClick: onBrandClick },
          { key: "current", label: "概览" },
        ]}
        onBrandClick={onBrandClick}
        onSidebarSelect={onSidebarSelect}
        onUserMenuClick={onUserMenuClick}
        pageTitle="概览"
        selectedSidebarKey="/dashboard"
        sidebarItems={[
          { key: "/dashboard", label: "概览" },
          { key: "/inventory", label: "库存", disabled: true },
        ]}
        sidebarTitle="后台导航"
        userInitial="A"
        userMenuItems={[{ key: "logout", label: "登出" }]}
        userName="admin"
      >
        <div>页面内容</div>
      </AppChrome>,
    );

    expect(screen.getByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.getByText("后台导航")).toBeInTheDocument();
    expect(screen.getByText("页面内容")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Lab Inventory 概览" }));
    expect(onBrandClick).toHaveBeenCalledTimes(1);

    const sideNavigation = screen.getByText("后台导航").closest(".app-sider-section");
    expect(sideNavigation).not.toBeNull();
    await user.click(within(sideNavigation as HTMLElement).getByText("概览"));
    expect(onSidebarSelect).toHaveBeenCalledWith("/dashboard");

    await user.click(screen.getByRole("button", { name: /admin/ }));
    await user.click(await screen.findByText("登出"));
    expect(onUserMenuClick).toHaveBeenCalledWith(expect.objectContaining({ key: "logout" }));
  });
});
