import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { testGuestUser, testLabAdminUser } from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

const unit = {
  allow_decimal: true,
  code: "kg",
  created_at: "2026-08-16T00:00:00Z",
  dimension: "mass",
  name: "千克",
  scale_to_base: 1,
  symbol: "kg",
  unit_id: "00000000-0000-4000-8000-0000000000e1",
};

/** A laboratory admin reaches its own units through /local rather than /admin. */
function withLocalUnits() {
  server.use(http.get("*/api/v1/local/units", () => HttpResponse.json([unit])));
}

describe("UnitsPage for a laboratory admin", () => {
  it("lists the laboratory's units", async () => {
    withLocalUnits();
    signIn(testLabAdminUser);
    renderRoute(["/admin/units"]);

    expect(await screen.findByText("千克")).toBeInTheDocument();
  });

  // The API has always allowed this; the UI used to hide unit management behind
  // a server-admin check, leaving a laboratory admin no way to add a unit.
  it("offers the management controls", async () => {
    withLocalUnits();
    signIn(testLabAdminUser);
    renderRoute(["/admin/units"]);

    const row = (await screen.findByText("千克")).closest("tr");
    expect(screen.getByRole("button", { name: "新建单位" })).toBeEnabled();
    expect(within(row as HTMLElement).getByRole("button", { name: "编辑" })).toBeEnabled();
    expect(within(row as HTMLElement).getByRole("button", { name: "删除" })).toBeEnabled();
  });

  it("keeps guests out of the page entirely", async () => {
    signIn(testGuestUser);
    renderRoute(["/admin/units"]);

    expect(await screen.findByRole("heading", { name: "无法访问" })).toBeInTheDocument();
  });
});
