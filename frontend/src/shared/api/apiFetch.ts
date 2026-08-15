import { isTauri } from "@tauri-apps/api/core";

/**
 * The `fetch` every backend call goes through.
 *
 * In a browser this is the platform one. Inside a Tauri shell it must not be:
 * the page is served from `http://tauri.localhost`, so a backend on any other
 * host is cross-site, and the `SameSite=Lax` session cookie the API issues is
 * never sent back — the login succeeds and the next request is a 401, which
 * drops the user straight back on the login page. The plugin runs the request
 * in Rust with its own cookie jar, outside the webview's cookie and CORS
 * rules, so the session survives and the backend needs no origin allowance for
 * the desktop app.
 *
 * The plugin is imported lazily so it stays out of the browser bundle.
 */
export async function apiFetch(input: string, init?: RequestInit) {
  if (!isTauri()) {
    return fetch(input, init);
  }

  const { fetch: tauriFetch } = await import("@tauri-apps/plugin-http");
  return tauriFetch(input, init);
}
