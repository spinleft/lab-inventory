import "@testing-library/jest-dom/vitest";
import { afterAll, afterEach, beforeAll, beforeEach } from "vitest";
import { server } from "./server";

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  server.resetHandlers();
});

afterAll(() => server.close());
