import { screen } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { renderRoute } from "../shared/test/render";
import { server } from "../shared/test/server";

describe("ProtectedRoute", () => {
  it("redirects anonymous users to login", async () => {
    server.use(
      http.get("*/api/v1/auth/me", () =>
        HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
      ),
    );

    renderRoute(["/assets"]);

    expect(await screen.findByRole("heading", { name: "登录" })).toBeInTheDocument();
  });

  it("renders protected content for authenticated users", async () => {
    renderRoute(["/assets"]);

    expect(await screen.findByRole("heading", { name: "资产" })).toBeInTheDocument();
    expect(await screen.findByText("示波器")).toBeInTheDocument();
  });
});
