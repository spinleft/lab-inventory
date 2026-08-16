import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import {
  testGuestUser,
  testLabAdminUser,
  testRegularUser,
  testRootUser,
} from "../../shared/test/fixtures";
import { configureBackend, renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

const CODE = "ABCD-1234";

function issuesCode(expiresInMinutes = 10) {
  server.use(
    http.post("*/api/v1/local/guest-registration-codes", () =>
      HttpResponse.json(
        {
          expires_at: new Date(Date.now() + expiresInMinutes * 60_000).toISOString(),
          laboratory_id: testLabAdminUser.laboratory?.laboratory_id,
          registration_code: CODE,
          registration_code_id: "00000000-0000-4000-8000-0000000000f1",
        },
        { status: 201 },
      ),
    ),
  );
}

describe("guest invitations", () => {
  it("hands a laboratory member a code with its expiry", async () => {
    issuesCode();
    // A plain member cannot open user administration, so their entry point is
    // the dashboard.
    signIn(testRegularUser);
    const { user } = renderRoute(["/dashboard"]);

    await user.click(await screen.findByRole("button", { name: "邀请访客" }));
    const dialog = within(await screen.findByRole("dialog"));
    await user.click(dialog.getByRole("button", { name: "生成注册码" }));

    expect(await screen.findByText(CODE)).toBeInTheDocument();
    expect(screen.getByText(/剩余有效时间/)).toBeInTheDocument();
    // Regenerating replaces the live code, so the wording has to change too.
    expect(dialog.getByRole("button", { name: "重新生成" })).toBeInTheDocument();
  });

  it("says the code is dead once it expires", async () => {
    issuesCode(-1);
    signIn(testLabAdminUser);
    const { user } = renderRoute(["/admin/users"]);

    await user.click(await screen.findByRole("button", { name: "邀请访客" }));
    await user.click(screen.getByRole("button", { name: "生成注册码" }));

    expect(await screen.findByText(/注册码已过期/)).toBeInTheDocument();
  });

  // The API requires the actor to belong to the laboratory the code is for, and
  // server admins belong to none — the button would only ever fail.
  it("is not offered to a system admin", async () => {
    signIn(testRootUser);
    renderRoute(["/admin/users"]);

    expect(await screen.findByRole("button", { name: "新建用户" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "邀请访客" })).not.toBeInTheDocument();
  });

  it("is not offered to a guest", async () => {
    signIn(testGuestUser);
    renderRoute(["/dashboard"]);

    expect(await screen.findByRole("heading", { name: "概览" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "邀请访客" })).not.toBeInTheDocument();
  });
});

describe("guest registration page", () => {
  function fillForm(user: ReturnType<typeof renderRoute>["user"]) {
    return Promise.all([]).then(async () => {
      await user.type(screen.getByLabelText("注册码"), CODE);
      await user.type(screen.getByLabelText("用户名"), "visitor");
      await user.type(screen.getByLabelText("密码"), "correct-horse-battery");
      await user.type(screen.getByLabelText("邮箱"), "visitor@example.com");
      await user.type(screen.getByLabelText("手机号"), "13800000000");
    });
  }

  it("registers and sends the visitor to the login page", async () => {
    let posted: unknown;
    server.use(
      http.post("*/api/v1/auth/guest-registration", async ({ request }) => {
        posted = await request.json();
        return HttpResponse.json({ user_id: "00000000-0000-4000-8000-0000000000f2" }, {
          status: 201,
        });
      }),
    );
    configureBackend();
    const { user } = renderRoute(["/register"]);

    await fillForm(user);
    await user.click(screen.getByRole("button", { name: "注册" }));

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(posted).toEqual({
      // The note is optional, and an untouched field is sent as null rather
      // than as an empty string the API would reject.
      description: null,
      email: "visitor@example.com",
      password: "correct-horse-battery",
      phone_number: "13800000000",
      registration_code: CODE,
      username: "visitor",
    });
  });

  it("sends the note the visitor wrote about themselves", async () => {
    let posted: { description?: unknown } | undefined;
    server.use(
      http.post("*/api/v1/auth/guest-registration", async ({ request }) => {
        posted = (await request.json()) as { description?: unknown };
        return HttpResponse.json({}, { status: 201 });
      }),
    );
    configureBackend();
    const { user } = renderRoute(["/register"]);

    await fillForm(user);
    await user.type(screen.getByLabelText(/备注/), "材料组李四，来借万用表");
    await user.click(screen.getByRole("button", { name: "注册" }));

    await screen.findByRole("heading", { name: "登录" });
    expect(posted?.description).toBe("材料组李四，来借万用表");
  });

  it("does not make the note mandatory", async () => {
    configureBackend();
    const { user } = renderRoute(["/register"]);

    await fillForm(user);

    expect(screen.getByRole("button", { name: "注册" })).toBeEnabled();
  });

  it("takes the code from the query string", () => {
    configureBackend();
    renderRoute([`/register?code=${CODE}`]);

    expect(screen.getByLabelText("注册码")).toHaveValue(CODE);
  });

  // Rate limiting is the one failure a visitor is likely to hit twice in a row.
  it("explains a rate limited attempt", async () => {
    server.use(
      http.post("*/api/v1/auth/guest-registration", () =>
        HttpResponse.json({ error: "Too many requests" }, { status: 429 }),
      ),
    );
    configureBackend();
    const { user } = renderRoute(["/register"]);

    await fillForm(user);
    await user.click(screen.getByRole("button", { name: "注册" }));

    expect(await screen.findByText(/尝试过于频繁/)).toBeInTheDocument();
  });

  it("keeps the submit button out of reach until every field is filled", async () => {
    configureBackend();
    const { user } = renderRoute(["/register"]);

    expect(screen.getByRole("button", { name: "注册" })).toBeDisabled();
    await user.type(screen.getByLabelText("注册码"), CODE);
    expect(screen.getByRole("button", { name: "注册" })).toBeDisabled();
  });
});

// Guards against the two pages drifting apart: the invite dialog tells people
// to look for this link.
describe("login page", () => {
  it("points visitors holding a code at registration", async () => {
    configureBackend();
    renderRoute(["/login"]);

    expect(
      await screen.findByRole("link", { name: "注册访客账号" }),
    ).toHaveAttribute("href", "/register");
  });
});
