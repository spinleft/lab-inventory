import { afterEach, describe, expect, it, vi } from "vitest";
import { normalizeApiBaseUrl, resolveDefaultApiBaseUrl } from "./backendConfig";

describe("normalizeApiBaseUrl", () => {
  it("adds /api/v1 to a host URL", () => {
    expect(normalizeApiBaseUrl("http://127.0.0.1:8000")).toBe(
      "http://127.0.0.1:8000/api/v1",
    );
  });

  it("keeps an existing /api/v1 suffix", () => {
    expect(normalizeApiBaseUrl("https://example.com/base/api/v1/")).toBe(
      "https://example.com/base/api/v1",
    );
  });

  it("rejects non-http protocols", () => {
    expect(() => normalizeApiBaseUrl("file:///tmp/api")).toThrow(
      "后端 API 地址必须使用 http 或 https。",
    );
  });
});

describe("resolveDefaultApiBaseUrl", () => {
  afterEach(() => {
    delete window.__LAB_INVENTORY_CONFIG__;
    vi.unstubAllEnvs();
  });

  it("falls back to the local development backend", () => {
    expect(resolveDefaultApiBaseUrl()).toBe("http://127.0.0.1:8000/api/v1");
  });

  it("prefers the build-time default over the fallback", () => {
    vi.stubEnv("VITE_DEFAULT_API_BASE_URL", "https://baked-in.example.com/api/v1");

    expect(resolveDefaultApiBaseUrl()).toBe("https://baked-in.example.com/api/v1");
  });

  it("prefers runtime configuration over the build-time default", () => {
    vi.stubEnv("VITE_DEFAULT_API_BASE_URL", "https://baked-in.example.com/api/v1");
    window.__LAB_INVENTORY_CONFIG__ = {
      apiBaseUrl: "https://deployed.example.com/api/v1",
    };

    expect(resolveDefaultApiBaseUrl()).toBe("https://deployed.example.com/api/v1");
  });

  // What the container writes when the API is reverse-proxied under the app's
  // own origin, which is the default deployment shape.
  it("resolves a path-only configuration against the page origin", () => {
    window.__LAB_INVENTORY_CONFIG__ = { apiBaseUrl: "/api/v1" };

    expect(resolveDefaultApiBaseUrl()).toBe(`${window.location.origin}/api/v1`);
  });

  // An unsubstituted or cleared config.js must not win over the build-time
  // default, or a Tauri bundle shipping the placeholder would lose its backend.
  it("ignores a blank runtime value", () => {
    vi.stubEnv("VITE_DEFAULT_API_BASE_URL", "https://baked-in.example.com/api/v1");
    window.__LAB_INVENTORY_CONFIG__ = { apiBaseUrl: "   " };

    expect(resolveDefaultApiBaseUrl()).toBe("https://baked-in.example.com/api/v1");
  });
});
