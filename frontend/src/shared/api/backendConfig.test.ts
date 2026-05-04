import { describe, expect, it } from "vitest";
import {
  BackendConfigError,
  normalizeApiBaseUrl,
  readStoredApiBaseUrl,
} from "./backendConfig";

describe("backend server configuration", () => {
  it("normalizes a server root URL into the API base URL", () => {
    expect(normalizeApiBaseUrl("http://example.test:8000")).toBe(
      "http://example.test:8000/api/v1",
    );
  });

  it("preserves an explicit /api/v1 URL and removes trailing slash", () => {
    expect(normalizeApiBaseUrl("https://inventory.example.com/api/v1/")).toBe(
      "https://inventory.example.com/api/v1",
    );
  });

  it("rejects unsupported protocols", () => {
    expect(() => normalizeApiBaseUrl("ftp://example.test")).toThrow(
      BackendConfigError,
    );
  });

  it("loads a saved backend URL from localStorage", () => {
    window.localStorage.setItem(
      "lab-inventory.backend-api-base-url",
      "http://lab.example.test:8080",
    );

    expect(readStoredApiBaseUrl()).toBe("http://lab.example.test:8080/api/v1");
  });
});
