import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { testLabAdminUser, testRootUser } from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

/**
 * A laboratory admin never reaches /admin/laboratories — the whole /admin scope
 * is closed to them — so the laboratory picker in the user editor has to come
 * from somewhere else. It used to come from that request and stay empty, which
 * made the form impossible to submit: the role requires a laboratory.
 */
function refuseLaboratoryListing() {
  server.use(
    http.get("*/api/v1/admin/laboratories", () =>
      HttpResponse.json({ error: "Forbidden" }, { status: 403 }),
    ),
  );
}

async function openNewUserForm(user: ReturnType<typeof renderRoute>["user"]) {
  await user.click(await screen.findByRole("button", { name: "新建用户" }));
  return within(await screen.findByRole("dialog"));
}

describe("UsersPage laboratory picker", () => {
  it("offers a laboratory admin their own laboratory", async () => {
    refuseLaboratoryListing();
    signIn(testLabAdminUser);
    const { user } = renderRoute(["/admin/users"]);

    const dialog = await openNewUserForm(user);
    await user.click(dialog.getByLabelText("实验室"));

    expect(
      await screen.findByRole("option", { name: testLabAdminUser.laboratory?.name }),
    ).toBeInTheDocument();
  });

  it("still lists every laboratory for a system admin", async () => {
    signIn(testRootUser);
    const { user } = renderRoute(["/admin/users"]);

    const dialog = await openNewUserForm(user);
    // Root's first creatable role is 超级管理员, which belongs to no
    // laboratory, so the picker only appears once the role needs one.
    await user.click(dialog.getByLabelText("角色"));
    await user.click(await screen.findByRole("option", { name: "普通用户" }));
    await user.click(dialog.getByLabelText("实验室"));

    // The fixtures carry more than one, which only a system admin ever sees.
    expect((await screen.findAllByRole("option")).length).toBeGreaterThan(1);
  });
});
