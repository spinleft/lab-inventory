import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { BACKEND_CONFIG_STORAGE_KEY } from "../../shared/api/backendConfig";
import {
  testCurrentUser,
  testMaintainerUser,
  testRegularUser,
} from "../../shared/test/fixtures";
import { renderRoute } from "../../shared/test/render";
import { server } from "../../shared/test/server";

vi.mock("antd", async (importOriginal) => {
  const actual = await importOriginal<typeof import("antd")>();
  return {
    ...actual,
    Drawer: ({ children, footer, open, title }: DrawerMockProps) =>
      open ? (
        <section aria-label={typeof title === "string" ? title : undefined} role="dialog">
          {typeof title === "string" ? <h2>{title}</h2> : title}
          {children}
          {footer}
        </section>
      ) : null,
  };
});

type DrawerMockProps = {
  children?: ReactNode;
  footer?: ReactNode;
  open?: boolean;
  title?: ReactNode;
};

type Laboratory = {
  laboratory_id: string;
  name: string;
  address: string;
  description: string | null;
  contact: string | null;
  created_at: string;
  updated_at: string;
};

type AdminUser = {
  user_id: string;
  username: string;
  email: string | null;
  user_type: {
    user_type_id: string;
    name: string;
  };
  laboratory: {
    laboratory_id: string;
    name: string;
  } | null;
  created_at: string;
  last_login_at: string | null;
};

const chemistryLaboratory: Laboratory = {
  laboratory_id: "00000000-0000-4000-8000-000000000013",
  name: "化学实验室",
  address: "A 座 301",
  description: "湿实验室",
  contact: "chem@example.com",
  created_at: "2026-05-01T00:00:00Z",
  updated_at: "2026-05-01T00:00:00Z",
};

const materialsLaboratory: Laboratory = {
  laboratory_id: "00000000-0000-4000-8000-000000000023",
  name: "材料实验室",
  address: "B 座 201",
  description: null,
  contact: "materials@example.com",
  created_at: "2026-05-02T00:00:00Z",
  updated_at: "2026-05-02T00:00:00Z",
};

const maintainerLaboratory: Laboratory = {
  ...chemistryLaboratory,
  laboratory_id: testMaintainerUser.laboratory.laboratory_id,
  name: testMaintainerUser.laboratory.name,
};

const ownerAdminUser: AdminUser = {
  user_id: testCurrentUser.user_id,
  username: testCurrentUser.username,
  email: testCurrentUser.email,
  user_type: testCurrentUser.user_type,
  laboratory: null,
  created_at: "2026-05-01T00:00:00Z",
  last_login_at: null,
};

const currentMaintainerAdminUser: AdminUser = {
  user_id: testMaintainerUser.user_id,
  username: testMaintainerUser.username,
  email: testMaintainerUser.email,
  user_type: testMaintainerUser.user_type,
  laboratory: testMaintainerUser.laboratory,
  created_at: "2026-05-01T00:00:00Z",
  last_login_at: null,
};

const peerMaintainerUser: AdminUser = {
  user_id: "00000000-0000-4000-8000-000000000041",
  username: "peer-maintainer",
  email: "peer@example.com",
  user_type: {
    user_type_id: "00000000-0000-4000-8000-000000000042",
    name: "maintainer",
  },
  laboratory: testMaintainerUser.laboratory,
  created_at: "2026-05-02T00:00:00Z",
  last_login_at: null,
};

const labUser: AdminUser = {
  user_id: "00000000-0000-4000-8000-000000000051",
  username: "lab-user",
  email: "lab-user@example.com",
  user_type: {
    user_type_id: "00000000-0000-4000-8000-000000000052",
    name: "user",
  },
  laboratory: {
    laboratory_id: materialsLaboratory.laboratory_id,
    name: materialsLaboratory.name,
  },
  created_at: "2026-05-03T00:00:00Z",
  last_login_at: "2026-05-04T00:00:00Z",
};

const ownLabUser: AdminUser = {
  ...labUser,
  user_id: "00000000-0000-4000-8000-000000000081",
  username: "own-lab-user",
  email: "own-lab-user@example.com",
  laboratory: testMaintainerUser.laboratory,
};

describe("AdminPages", () => {
  it("allows admin center for maintainers with laboratory scope messaging", async () => {
    renderAuthenticatedRoute("/admin", testMaintainerUser);
    expect((await screen.findAllByText("管理中心")).length).toBeGreaterThan(0);
    expect(screen.getByText("你只能管理自己实验室范围内的数据。")).toBeInTheDocument();
  });

  it("lets owners create, edit, and delete laboratories", async () => {
    let laboratories = [chemistryLaboratory, materialsLaboratory];
    let createBody: unknown;
    let updateBody: unknown;
    let deletedLaboratoryId: string | undefined;

    server.use(
      http.get("*/api/v1/laboratories", () => HttpResponse.json(laboratories)),
      http.post("*/api/v1/laboratories", async ({ request }) => {
        createBody = await request.json();
        const created: Laboratory = {
          laboratory_id: "00000000-0000-4000-8000-000000000033",
          name: "物理实验室",
          address: "C 座 101",
          description: null,
          contact: null,
          created_at: "2026-05-03T00:00:00Z",
          updated_at: "2026-05-03T00:00:00Z",
        };
        laboratories = [...laboratories, created];
        return HttpResponse.json(created, { status: 201 });
      }),
      http.patch("*/api/v1/laboratories/:laboratoryId", async ({ params, request }) => {
        updateBody = await request.json();
        laboratories = laboratories.map((laboratory) =>
          laboratory.laboratory_id === params.laboratoryId
            ? {
                ...laboratory,
                name: "化学实验室东区",
                address: "A 座 302",
                description: null,
                contact: "east@example.com",
              }
            : laboratory,
        );
        return HttpResponse.json(
          laboratories.find(
            (laboratory) => laboratory.laboratory_id === params.laboratoryId,
          ),
        );
      }),
      http.delete("*/api/v1/laboratories/:laboratoryId", ({ params }) => {
        deletedLaboratoryId = String(params.laboratoryId);
        laboratories = laboratories.filter(
          (laboratory) => laboratory.laboratory_id !== params.laboratoryId,
        );
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderAuthenticatedRoute("/admin/laboratories", testCurrentUser);
    expect(await screen.findByRole("heading", { level: 1, name: "实验室" })).toBeInTheDocument();
    expect(await screen.findByText("化学实验室")).toBeInTheDocument();
    expect(screen.getByText("材料实验室")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "新增实验室" }));
    fireEvent.change(await screen.findByLabelText("实验室名称"), {
      target: { value: "物理实验室" },
    });
    fireEvent.change(screen.getByLabelText("地址"), {
      target: { value: "C 座 101" },
    });
    fireEvent.submit(screen.getByRole("form", { name: "实验室表单" }));

    await waitFor(() =>
      expect(createBody).toEqual({
        address: "C 座 101",
        contact: null,
        description: null,
        name: "物理实验室",
      }),
    );
    expect(await screen.findByText("物理实验室")).toBeInTheDocument();

    const chemistryRow = screen.getByText("化学实验室").closest("tr");
    expect(chemistryRow).not.toBeNull();
    fireEvent.click(
      within(chemistryRow as HTMLElement).getByRole("button", { name: "编辑" }),
    );
    const nameInput = await screen.findByLabelText("实验室名称");
    fireEvent.change(nameInput, { target: { value: "化学实验室东区" } });
    const addressInput = screen.getByLabelText("地址");
    fireEvent.change(addressInput, { target: { value: "A 座 302" } });
    fireEvent.change(screen.getByLabelText("描述"), { target: { value: "" } });
    const contactInput = screen.getByLabelText("联系方式");
    fireEvent.change(contactInput, { target: { value: "east@example.com" } });
    fireEvent.submit(screen.getByRole("form", { name: "实验室表单" }));

    await waitFor(() =>
      expect(updateBody).toEqual({
        address: "A 座 302",
        contact: "east@example.com",
        description: null,
        name: "化学实验室东区",
      }),
    );
    expect(await screen.findByText("化学实验室东区")).toBeInTheDocument();

    const materialsRow = screen.getByText("材料实验室").closest("tr");
    expect(materialsRow).not.toBeNull();
    fireEvent.click(
      within(materialsRow as HTMLElement).getByRole("button", { name: "删除" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "确认删除" }));

    await waitFor(() =>
      expect(deletedLaboratoryId).toBe(materialsLaboratory.laboratory_id),
    );
    await waitFor(() =>
      expect(screen.queryByText("材料实验室")).not.toBeInTheDocument(),
    );
  }, 10_000);

  it("limits maintainers to editing their own laboratory", async () => {
    let updateBody: unknown;
    server.use(
      http.get("*/api/v1/laboratories", () =>
        HttpResponse.json([maintainerLaboratory]),
      ),
      http.patch("*/api/v1/laboratories/:laboratoryId", async ({ request }) => {
        updateBody = await request.json();
        return HttpResponse.json({
          ...maintainerLaboratory,
          name: "化学实验室东区",
          address: "A 座 302",
          description: "湿实验室",
          contact: "east@example.com",
        });
      }),
    );

    renderAuthenticatedRoute("/admin/laboratories", testMaintainerUser);
    expect(await screen.findByText("化学实验室")).toBeInTheDocument();
    expect(screen.queryByText("材料实验室")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "新增实验室" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "编辑" }));
    const nameInput = await screen.findByLabelText("实验室名称");
    fireEvent.change(nameInput, { target: { value: "化学实验室东区" } });
    const addressInput = screen.getByLabelText("地址");
    fireEvent.change(addressInput, { target: { value: "A 座 302" } });
    const contactInput = screen.getByLabelText("联系方式");
    fireEvent.change(contactInput, { target: { value: "east@example.com" } });
    fireEvent.submit(screen.getByRole("form", { name: "实验室表单" }));

    await waitFor(() =>
      expect(updateBody).toEqual({
        address: "A 座 302",
        contact: "east@example.com",
        description: "湿实验室",
        name: "化学实验室东区",
      }),
    );
  });

  it("shows an empty state when a maintainer has no laboratory", async () => {
    server.use(http.get("*/api/v1/laboratories", () => HttpResponse.json([])));
    renderAuthenticatedRoute("/admin/laboratories", {
      ...testMaintainerUser,
      laboratory: null,
    });

    expect(await screen.findByText("当前账号未绑定实验室。")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "编辑" })).not.toBeInTheDocument();
  });

  it("lets owners create, edit, and delete users", async () => {
    let users = [ownerAdminUser, peerMaintainerUser, labUser];
    let createBody: unknown;
    let updateBody: unknown;
    let deletedUserId: string | undefined;

    server.use(
      http.get("*/api/v1/laboratories", () =>
        HttpResponse.json([chemistryLaboratory, materialsLaboratory]),
      ),
      http.get("*/api/v1/users", () => HttpResponse.json(users)),
      http.post("*/api/v1/users", async ({ request }) => {
        createBody = await request.json();
        const created: AdminUser = {
          user_id: "00000000-0000-4000-8000-000000000061",
          username: "new-lab-user",
          email: "new@example.com",
          user_type: labUser.user_type,
          laboratory: {
            laboratory_id: chemistryLaboratory.laboratory_id,
            name: chemistryLaboratory.name,
          },
          created_at: "2026-05-05T00:00:00Z",
          last_login_at: null,
        };
        users = [...users, created];
        return HttpResponse.json(created, { status: 201 });
      }),
      http.patch("*/api/v1/users/:userId", async ({ params, request }) => {
        updateBody = await request.json();
        users = users.map((user) =>
          user.user_id === params.userId
            ? {
                ...user,
                username: "lab-user-east",
                email: null,
              }
            : user,
        );
        return HttpResponse.json(users.find((user) => user.user_id === params.userId));
      }),
      http.delete("*/api/v1/users/:userId", ({ params }) => {
        deletedUserId = String(params.userId);
        users = users.filter((user) => user.user_id !== params.userId);
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderAuthenticatedRoute("/admin/users", testCurrentUser);
    expect(await screen.findByRole("heading", { level: 1, name: "用户" })).toBeInTheDocument();
    expect(await screen.findByText("peer-maintainer")).toBeInTheDocument();
    expect(screen.getByText("lab-user")).toBeInTheDocument();
    const ownerRow = getTableRowByText("admin");
    expect(
      within(ownerRow as HTMLElement).getByRole("button", { name: "删除" }),
    ).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "新增用户" }));
    fireEvent.change(await screen.findByLabelText("用户名"), {
      target: { value: "new-lab-user" },
    });
    fireEvent.change(screen.getByLabelText("邮箱"), {
      target: { value: "new@example.com" },
    });
    fireEvent.change(screen.getByLabelText("密码"), {
      target: { value: "initial-pass" },
    });
    fireEvent.submit(screen.getByRole("form", { name: "用户表单" }));

    await waitFor(() =>
      expect(createBody).toEqual({
        email: "new@example.com",
        laboratory_id: chemistryLaboratory.laboratory_id,
        password: "initial-pass",
        user_type: "user",
        username: "new-lab-user",
      }),
    );
    expect(await screen.findByText("new-lab-user")).toBeInTheDocument();

    const userRow = screen.getByText("lab-user").closest("tr");
    expect(userRow).not.toBeNull();
    fireEvent.click(within(userRow as HTMLElement).getByRole("button", { name: "编辑" }));
    fireEvent.change(await screen.findByLabelText("用户名"), {
      target: { value: "lab-user-east" },
    });
    fireEvent.change(screen.getByLabelText("邮箱"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "reset-pass" } });
    fireEvent.submit(screen.getByRole("form", { name: "用户表单" }));

    await waitFor(() =>
      expect(updateBody).toEqual({
        email: null,
        laboratory_id: materialsLaboratory.laboratory_id,
        password: "reset-pass",
        user_type: "user",
        username: "lab-user-east",
      }),
    );
    expect(await screen.findByText("lab-user-east")).toBeInTheDocument();

    const updatedUserRow = screen.getByText("lab-user-east").closest("tr");
    expect(updatedUserRow).not.toBeNull();
    fireEvent.click(
      within(updatedUserRow as HTMLElement).getByRole("button", { name: "删除" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "确认删除" }));

    await waitFor(() => expect(deletedUserId).toBe(labUser.user_id));
  }, 10_000);

  it("lets maintainers manage other maintainers in their own laboratory", async () => {
    let users = [currentMaintainerAdminUser, peerMaintainerUser, ownLabUser];
    let createBody: unknown;
    let updateBody: unknown;
    let promotionBody: unknown;
    let deletedUserId: string | undefined;

    server.use(
      http.get("*/api/v1/users", () => HttpResponse.json(users)),
      http.post("*/api/v1/users", async ({ request }) => {
        createBody = await request.json();
        const created: AdminUser = {
          user_id: "00000000-0000-4000-8000-000000000071",
          username: "new-maintainer",
          email: null,
          user_type: peerMaintainerUser.user_type,
          laboratory: testMaintainerUser.laboratory,
          created_at: "2026-05-05T00:00:00Z",
          last_login_at: null,
        };
        users = [...users, created];
        return HttpResponse.json(created, { status: 201 });
      }),
      http.patch("*/api/v1/users/:userId", async ({ params, request }) => {
        const body = await request.json();
        if (params.userId === ownLabUser.user_id) {
          promotionBody = body;
        } else {
          updateBody = body;
        }
        users = users.map((user) =>
          user.user_id === params.userId
            ? {
                ...user,
                username:
                  params.userId === ownLabUser.user_id
                    ? "own-lab-user"
                    : "peer-maintainer-east",
                user_type:
                  params.userId === ownLabUser.user_id
                    ? peerMaintainerUser.user_type
                    : user.user_type,
              }
            : user,
        );
        return HttpResponse.json(users.find((user) => user.user_id === params.userId));
      }),
      http.delete("*/api/v1/users/:userId", ({ params }) => {
        deletedUserId = String(params.userId);
        users = users.filter((user) => user.user_id !== params.userId);
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderAuthenticatedRoute("/admin/users", testMaintainerUser);
    expect(await screen.findByText("peer-maintainer")).toBeInTheDocument();
    expect(screen.getByText("own-lab-user")).toBeInTheDocument();
    expect(screen.queryByText("admin")).not.toBeInTheDocument();
    const currentUserRow = getTableRowByText("maintainer");
    expect(
      within(currentUserRow as HTMLElement).getByRole("button", { name: "删除" }),
    ).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "新增用户" }));
    expect(await screen.findByLabelText("实验室维护者")).toBeInTheDocument();
    expect(screen.queryByLabelText("系统所有者")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("实验室")).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("实验室维护者"));
    fireEvent.change(screen.getByLabelText("用户名"), {
      target: { value: "new-maintainer" },
    });
    fireEvent.change(screen.getByLabelText("密码"), {
      target: { value: "initial-pass" },
    });
    fireEvent.submit(screen.getByRole("form", { name: "用户表单" }));

    await waitFor(() =>
      expect(createBody).toEqual({
        email: null,
        laboratory_id: testMaintainerUser.laboratory.laboratory_id,
        password: "initial-pass",
        user_type: "maintainer",
        username: "new-maintainer",
      }),
    );

    const ownLabUserRow = screen.getByText("own-lab-user").closest("tr");
    expect(ownLabUserRow).not.toBeNull();
    fireEvent.click(
      within(ownLabUserRow as HTMLElement).getByRole("button", { name: "编辑" }),
    );
    fireEvent.click(await screen.findByLabelText("实验室维护者"));
    fireEvent.submit(screen.getByRole("form", { name: "用户表单" }));

    await waitFor(() =>
      expect(promotionBody).toEqual({
        email: "own-lab-user@example.com",
        laboratory_id: testMaintainerUser.laboratory.laboratory_id,
        user_type: "maintainer",
        username: "own-lab-user",
      }),
    );

    const peerRow = screen.getByText("peer-maintainer").closest("tr");
    expect(peerRow).not.toBeNull();
    fireEvent.click(within(peerRow as HTMLElement).getByRole("button", { name: "编辑" }));
    fireEvent.change(await screen.findByLabelText("用户名"), {
      target: { value: "peer-maintainer-east" },
    });
    fireEvent.submit(screen.getByRole("form", { name: "用户表单" }));

    await waitFor(() =>
      expect(updateBody).toEqual({
        email: "peer@example.com",
        laboratory_id: testMaintainerUser.laboratory.laboratory_id,
        user_type: "maintainer",
        username: "peer-maintainer-east",
      }),
    );

    const updatedPeerRow = screen.getByText("peer-maintainer-east").closest("tr");
    expect(updatedPeerRow).not.toBeNull();
    fireEvent.click(
      within(updatedPeerRow as HTMLElement).getByRole("button", { name: "删除" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "确认删除" }));

    await waitFor(() => expect(deletedUserId).toBe(peerMaintainerUser.user_id));
  }, 10_000);

  it("shows an empty user state when a maintainer has no laboratory", async () => {
    server.use(http.get("*/api/v1/users", () => HttpResponse.json([])));
    renderAuthenticatedRoute("/admin/users", {
      ...testMaintainerUser,
      laboratory: null,
    });

    expect(await screen.findByText("当前账号未绑定实验室。")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "新增用户" })).not.toBeInTheDocument();
  });

  it("redirects the singular admin user route", async () => {
    server.use(http.get("*/api/v1/users", () => HttpResponse.json([ownerAdminUser])));
    renderAuthenticatedRoute("/admin/user", testCurrentUser);
    expect(await screen.findByRole("heading", { level: 1, name: "用户" })).toBeInTheDocument();
  });

  it("blocks admin center for regular users", async () => {
    renderAuthenticatedRoute("/admin", testRegularUser);
    expect(await screen.findByText("无权限访问")).toBeInTheDocument();
  });
});

type TestUser =
  | typeof testCurrentUser
  | typeof testMaintainerUser
  | typeof testRegularUser
  | (Omit<typeof testMaintainerUser, "laboratory"> & { laboratory: null });

function renderAuthenticatedRoute(route: string, currentUser: TestUser) {
  window.localStorage.setItem(
    BACKEND_CONFIG_STORAGE_KEY,
    "http://127.0.0.1:8000/api/v1",
  );
  server.use(http.get("*/api/v1/auth/me", () => HttpResponse.json(currentUser)));
  renderRoute([route]);
}

function getTableRowByText(text: string) {
  const row = screen.getAllByText(text).find((element) => element.closest("tr"));
  expect(row).toBeDefined();
  return row?.closest("tr") as HTMLElement;
}
