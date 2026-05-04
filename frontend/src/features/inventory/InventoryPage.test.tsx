import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderRoute } from "../../shared/test/render";

describe("InventoryPage", () => {
  it("renders inventory items from the API", async () => {
    renderRoute(["/inventory"]);

    expect(await screen.findByRole("heading", { name: "库存" })).toBeInTheDocument();
    expect(await screen.findByText("SN-001")).toBeInTheDocument();
    expect(screen.getByText("A-101")).toBeInTheDocument();
  });
});
