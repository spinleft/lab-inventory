import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";
import {
  testLaboratories,
  testManagedUsers,
  testUser,
} from "../../shared/test/handlers";

describe("AdminPage", () => {
  it("adds an admin area entry for owners", async () => {
    const user = userEvent.setup();
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "管理区" }));

    expect(await screen.findByRole("heading", { name: "管理区" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "用户" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "实验室" })).toBeInTheDocument();
    expect(await screen.findByText("materials-user")).toBeInTheDocument();
  });

  it("creates, updates, and deletes users as an owner", async () => {
    const user = userEvent.setup();
    let createPayload: unknown;
    let updatePayload: unknown;
    let deletedUserId: string | readonly string[] | undefined;
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    server.use(
      http.post("*/api/v1/users", async ({ request }) => {
        createPayload = await request.json();
        return HttpResponse.json(
          {
            user_id: "00000000-0000-0000-0000-000000000099",
            username: "lab-assistant",
            email: "assistant@example.com",
            user_type: {
              user_type_id: "00000000-0000-0000-0000-000000000004",
              name: "user",
            },
            laboratory: {
              laboratory_id: testLaboratories[1].laboratory_id,
              name: testLaboratories[1].name,
            },
            created_at: "2026-05-03T00:00:00Z",
            last_login_at: null,
          },
          { status: 201 },
        );
      }),
      http.patch("*/api/v1/users/:userId", async ({ request }) => {
        updatePayload = await request.json();
        return HttpResponse.json({
          ...testManagedUsers[1],
          email: "updated@example.com",
        });
      }),
      http.delete("*/api/v1/users/:userId", ({ params }) => {
        deletedUserId = params.userId;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderRoute(["/admin/users"]);

    expect(await screen.findByRole("heading", { name: "管理区" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "新建用户" }));
    await user.type(screen.getByLabelText("用户名"), "lab-assistant");
    await user.type(screen.getByLabelText("邮箱"), "assistant@example.com");
    await user.type(screen.getByLabelText("初始密码"), "password");
    await screen.findByRole("option", { name: "物理实验室" });
    await user.selectOptions(
      screen.getByLabelText("实验室"),
      testLaboratories[1].laboratory_id,
    );
    await user.click(screen.getByRole("button", { name: "创建用户" }));

    expect(await screen.findByText("用户已创建。")).toBeInTheDocument();
    expect(createPayload).toEqual({
      username: "lab-assistant",
      password: "password",
      user_type: "user",
      email: "assistant@example.com",
      laboratory_id: testLaboratories[1].laboratory_id,
    });

    const userRow = screen.getByText("materials-user").closest("tr");
    expect(userRow).not.toBeNull();
    await user.click(within(userRow!).getByRole("button", { name: "修改" }));
    const emailInput = screen.getByLabelText("邮箱");
    await user.clear(emailInput);
    await user.type(emailInput, "updated@example.com");
    await user.click(screen.getByRole("button", { name: "保存修改" }));

    expect(await screen.findByText("用户已更新。")).toBeInTheDocument();
    expect(updatePayload).toMatchObject({
      username: "materials-user",
      email: "updated@example.com",
      user_type: "user",
      laboratory_id: testLaboratories[0].laboratory_id,
    });

    const updatedUserRow = screen.getByText("materials-user").closest("tr");
    expect(updatedUserRow).not.toBeNull();
    await user.click(within(updatedUserRow!).getByRole("button", { name: "删除" }));

    expect(await screen.findByText("用户已删除。")).toBeInTheDocument();
    expect(deletedUserId).toBe(testManagedUsers[1].user_id);
    confirm.mockRestore();
  });

  it("creates, updates, and deletes laboratories as an owner", async () => {
    const user = userEvent.setup();
    let createPayload: unknown;
    let updatePayload: unknown;
    let deletedLaboratoryId: string | readonly string[] | undefined;
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    server.use(
      http.post("*/api/v1/laboratories", async ({ request }) => {
        createPayload = await request.json();
        return HttpResponse.json(
          {
            laboratory_id: "20000000-0000-0000-0000-000000000099",
            name: "光学实验室",
            address: "C-301",
            description: "",
            contact: "optics@example.com",
            created_at: "2026-05-03T00:00:00Z",
            updated_at: "2026-05-03T00:00:00Z",
          },
          { status: 201 },
        );
      }),
      http.patch("*/api/v1/laboratories/:laboratoryId", async ({ request }) => {
        updatePayload = await request.json();
        return HttpResponse.json({
          ...testLaboratories[0],
          address: "A-102",
        });
      }),
      http.delete("*/api/v1/laboratories/:laboratoryId", ({ params }) => {
        deletedLaboratoryId = params.laboratoryId;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderRoute(["/admin/laboratories"]);

    expect(await screen.findByRole("heading", { name: "管理区" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "新建实验室" }));
    await user.type(screen.getByLabelText("名称"), "光学实验室");
    await user.type(screen.getByLabelText("地址"), "C-301");
    await user.type(screen.getByLabelText("联系人"), "optics@example.com");
    await user.click(screen.getByRole("button", { name: "创建实验室" }));

    expect(await screen.findByText("实验室已创建。")).toBeInTheDocument();
    expect(createPayload).toEqual({
      name: "光学实验室",
      address: "C-301",
      description: "",
      contact: "optics@example.com",
    });

    const laboratoryRow = screen.getByText("材料实验室").closest("tr");
    expect(laboratoryRow).not.toBeNull();
    await user.click(within(laboratoryRow!).getByRole("button", { name: "修改" }));
    const addressInput = screen.getByLabelText("地址");
    await user.clear(addressInput);
    await user.type(addressInput, "A-102");
    await user.click(screen.getByRole("button", { name: "保存修改" }));

    expect(await screen.findByText("实验室已更新。")).toBeInTheDocument();
    expect(updatePayload).toMatchObject({
      name: "材料实验室",
      address: "A-102",
      contact: "materials@example.com",
    });

    const updatedLaboratoryRow = screen.getByText("材料实验室").closest("tr");
    expect(updatedLaboratoryRow).not.toBeNull();
    await user.click(
      within(updatedLaboratoryRow!).getByRole("button", { name: "删除" }),
    );

    expect(await screen.findByText("实验室已删除。")).toBeInTheDocument();
    expect(deletedLaboratoryId).toBe(testLaboratories[0].laboratory_id);
    confirm.mockRestore();
  });

  it("limits maintainers to their laboratory users", async () => {
    const user = userEvent.setup();
    const maintainer = {
      ...testUser,
      user_type: {
        user_type_id: "00000000-0000-0000-0000-000000000005",
        name: "maintainer",
      },
      laboratory: {
        laboratory_id: testLaboratories[0].laboratory_id,
        name: testLaboratories[0].name,
      },
    };
    let createPayload: unknown;
    server.use(
      http.get("*/api/v1/auth/me", () => HttpResponse.json(maintainer)),
      http.get("*/api/v1/users", () =>
        HttpResponse.json([
          {
            ...testManagedUsers[1],
            laboratory: maintainer.laboratory,
          },
        ]),
      ),
      http.post("*/api/v1/users", async ({ request }) => {
        createPayload = await request.json();
        return HttpResponse.json(
          {
            ...testManagedUsers[1],
            username: "lab-user",
            laboratory: maintainer.laboratory,
          },
          { status: 201 },
        );
      }),
    );

    renderRoute(["/admin/users"]);

    expect(await screen.findByRole("heading", { name: "管理区" })).toBeInTheDocument();
    expect(screen.getByText("材料实验室 用户")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "实验室" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "新建用户" }));
    expect(screen.queryByRole("option", { name: "管理员" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "实验室维护者" }),
    ).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("用户名"), "lab-user");
    await user.type(screen.getByLabelText("初始密码"), "password");
    await user.click(screen.getByRole("button", { name: "创建用户" }));

    expect(await screen.findByText("用户已创建。")).toBeInTheDocument();
    expect(createPayload).toEqual({
      username: "lab-user",
      password: "password",
      user_type: "user",
    });
  });

  it("blocks laboratory management for maintainers", async () => {
    server.use(
      http.get("*/api/v1/auth/me", () =>
        HttpResponse.json({
          ...testUser,
          user_type: {
            user_type_id: "00000000-0000-0000-0000-000000000005",
            name: "maintainer",
          },
          laboratory: {
            laboratory_id: testLaboratories[0].laboratory_id,
            name: testLaboratories[0].name,
          },
        }),
      ),
    );

    renderRoute(["/admin/laboratories"]);

    expect(
      await screen.findByRole("heading", { name: "管理区" }),
    ).toBeInTheDocument();
    expect(screen.getByText("无法访问实验室管理")).toBeInTheDocument();
  });
});
