import { describe, expect, it } from "vitest";
import { normalizeApiBaseUrl } from "./backendConfig";

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
