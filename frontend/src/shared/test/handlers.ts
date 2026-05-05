import { http, HttpResponse } from "msw";

export const handlers = [
  http.get("*/api/v1/health_check", () => HttpResponse.json({ status: "ok" })),
  http.get("*/api/v1/auth/me", () =>
    HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
  ),
  http.post("*/api/v1/auth/logout", () =>
    HttpResponse.json({ message: "Logout successful" }),
  ),
  http.patch("*/api/v1/auth/password", () =>
    HttpResponse.json({ message: "Password changed" }),
  ),
];
