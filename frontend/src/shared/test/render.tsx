import { render, type RenderOptions } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { type ReactElement } from "react";
import { MemoryRouter } from "react-router-dom";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";
import { type CurrentUser } from "../../modules/auth/types";
import { BACKEND_CONFIG_STORAGE_KEY } from "../api/backendConfig";
import { server } from "./server";

export const TEST_API_BASE_URL = "http://127.0.0.1:8000/api/v1";

/**
 * Marks the backend as configured. `BackendConfigProvider` reads localStorage
 * once at mount, so this has to run before render, not after.
 */
export function configureBackend(apiBaseUrl = TEST_API_BASE_URL) {
  window.localStorage.setItem(BACKEND_CONFIG_STORAGE_KEY, apiBaseUrl);
}

/** Makes `/auth/me` resolve to `user`, i.e. an authenticated session. */
export function signIn(user: CurrentUser) {
  configureBackend();
  server.use(http.get("*/api/v1/auth/me", () => HttpResponse.json(user)));
}

type Options = RenderOptions & { initialEntries?: string[] };

/**
 * Renders `ui` inside the same provider stack `main.tsx` uses, so tests cover
 * the real theme/toast/query/backend-config wiring rather than a stand-in.
 */
export function renderApp(ui: ReactElement, { initialEntries = ["/"], ...options }: Options = {}) {
  return {
    user: userEvent.setup(),
    ...render(
      <MemoryRouter initialEntries={initialEntries}>
        <AppProviders>{ui}</AppProviders>
      </MemoryRouter>,
      options,
    ),
  };
}

/**
 * Renders the whole routed application at `initialEntries`. Use this for
 * anything that depends on routing, the auth gate, or the app shell.
 */
export function renderRoute(initialEntries: string[] = ["/"], options?: RenderOptions) {
  return renderApp(<App />, { ...options, initialEntries });
}
