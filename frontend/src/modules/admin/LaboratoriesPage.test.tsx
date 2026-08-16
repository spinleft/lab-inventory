import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import {
  testLabAdminUser,
  testLaboratories,
  testRootUser,
} from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

/** /admin is closed to a laboratory admin, whatever the page asks for. */
function refuseLaboratoryListing() {
  server.use(
    http.get("*/api/v1/admin/laboratories", () =>
      HttpResponse.json({ error: "Forbidden" }, { status: 403 }),
    ),
  );
}

describe("LaboratoriesPage for a laboratory admin", () => {
  it("shows the one laboratory the API will hand them", async () => {
    refuseLaboratoryListing();
    signIn(testLabAdminUser);
    renderRoute(["/admin/laboratories"]);

    expect(await screen.findByText("化学实验室")).toBeInTheDocument();
    // The other laboratory in the fixtures is none of their business.
    expect(screen.queryByText("材料实验室")).not.toBeInTheDocument();
  });

  it("lets them edit it but not create or delete one", async () => {
    refuseLaboratoryListing();
    signIn(testLabAdminUser);
    renderRoute(["/admin/laboratories"]);

    const row = (await screen.findByText("化学实验室")).closest("tr") as HTMLElement;
    expect(within(row).getByRole("button", { name: "编辑" })).toBeEnabled();
    expect(within(row).queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "新建实验室" })).not.toBeInTheDocument();
  });

  it("saves the edit through the laboratory-scoped route", async () => {
    refuseLaboratoryListing();
    let patchedPath: string | undefined;
    // Stateful, so the refetch that follows the save sees the new name — the
    // page reads the list back rather than trusting the mutation's response.
    let name = "化学实验室";
    server.use(
      http.get("*/api/v1/local/laboratory", () =>
        HttpResponse.json({ ...testLaboratories[0], name }),
      ),
      http.patch("*/api/v1/local/laboratory", ({ request }) => {
        patchedPath = new URL(request.url).pathname;
        name = "化学实验室 II";
        return HttpResponse.json({ ...testLaboratories[0], name });
      }),
    );
    signIn(testLabAdminUser);
    const { user } = renderRoute(["/admin/laboratories"]);

    const row = (await screen.findByText("化学实验室")).closest("tr") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: "编辑" }));
    const dialog = within(await screen.findByRole("dialog"));
    await user.clear(dialog.getByLabelText("名称"));
    await user.type(dialog.getByLabelText("名称"), "化学实验室 II");
    await user.click(dialog.getByRole("button", { name: "保存" }));

    // /admin/laboratories/{id} would have been refused.
    await screen.findByText("化学实验室 II");
    expect(patchedPath).toBe("/api/v1/local/laboratory");
  });
});

describe("LaboratoriesPage for a system admin", () => {
  it("still lists every laboratory", async () => {
    signIn(testRootUser);
    renderRoute(["/admin/laboratories"]);

    expect(await screen.findByText("化学实验室")).toBeInTheDocument();
    expect(screen.getByText("材料实验室")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "新建实验室" })).toBeInTheDocument();
  });
});
