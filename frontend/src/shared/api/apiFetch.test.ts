import { http, HttpResponse } from "msw";
import { afterEach, describe, expect, it, vi } from "vitest";
import { server } from "../test/server";
import { apiFetch } from "./apiFetch";

const tauriFetch = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: tauriFetch }));

afterEach(() => {
  delete (globalThis as { isTauri?: boolean }).isTauri;
  tauriFetch.mockReset();
});

describe("apiFetch", () => {
  it("uses the platform fetch in a browser", async () => {
    server.use(
      http.get("http://api.test/api/v1/ping", () => HttpResponse.json({ ok: true })),
    );

    const response = await apiFetch("http://api.test/api/v1/ping");

    await expect(response.json()).resolves.toEqual({ ok: true });
    expect(tauriFetch).not.toHaveBeenCalled();
  });

  // The webview would send this one cross-site from http://tauri.localhost and
  // strip the SameSite=Lax session cookie, so it has to go through the plugin.
  it("routes through the http plugin inside a Tauri shell", async () => {
    (globalThis as { isTauri?: boolean }).isTauri = true;
    tauriFetch.mockResolvedValue(HttpResponse.json({ ok: true }));

    const response = await apiFetch("http://api.test/api/v1/ping", {
      credentials: "include",
      method: "GET",
    });

    await expect(response.json()).resolves.toEqual({ ok: true });
    expect(tauriFetch).toHaveBeenCalledWith("http://api.test/api/v1/ping", {
      credentials: "include",
      method: "GET",
    });
  });
});
