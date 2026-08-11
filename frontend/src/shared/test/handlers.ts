import { http, HttpResponse } from "msw";
import { testLaboratories } from "./fixtures";

/**
 * Collections the backend returns inside a pagination envelope. Everything else
 * is a bare array, and Zod rejects the wrong shape, so this split has to match
 * the schemas in `src/modules/*\/api.ts`.
 */
const PAGINATED_COLLECTIONS = new Set(["assets", "inventory-items", "borrow-requests"]);

export function emptyCollection(collection: string) {
  return PAGINATED_COLLECTIONS.has(collection)
    ? { items: [], limit: 20, offset: 0, total: 0 }
    : [];
}

function collectionResponse({ params }: { params: Record<string, unknown> }) {
  return HttpResponse.json(emptyCollection(String(params.collection ?? "")));
}

/**
 * Defaults describe a signed-out client talking to an otherwise empty backend.
 * Tests override with `server.use(...)`; the suite runs with
 * `onUnhandledRequest: "error"`, so anything missing here fails loudly.
 */
export const handlers = [
  http.get("*/api/v1/health_check", () => HttpResponse.json({ status: "ok" })),

  http.get("*/api/v1/auth/me", () =>
    HttpResponse.json({ error: "Authentication required" }, { status: 401 }),
  ),
  http.post("*/api/v1/auth/login", () => HttpResponse.json({ message: "Login successful" })),
  http.post("*/api/v1/auth/logout", () =>
    HttpResponse.json({ message: "Logout successful" }),
  ),
  http.patch("*/api/v1/auth/password", () =>
    HttpResponse.json({ message: "Password changed" }),
  ),

  // System-admin scope: /admin/laboratories/{id}/{collection}
  http.get("*/api/v1/admin/laboratories", () => HttpResponse.json(testLaboratories)),
  http.get("*/api/v1/admin/users", () => HttpResponse.json([])),
  http.get("*/api/v1/admin/laboratories/:laboratoryId/:collection", collectionResponse),
  http.get(
    "*/api/v1/admin/laboratories/:laboratoryId/federation/:collection",
    collectionResponse,
  ),

  // Laboratory-scoped users: /local/{collection}
  http.get("*/api/v1/local/users", () => HttpResponse.json([])),
  http.get("*/api/v1/local/:collection", collectionResponse),
  http.get("*/api/v1/local/federation/:collection", collectionResponse),

  // Federated remote scope
  http.get(
    "*/api/v1/federation/nodes/:nodeId/laboratories/:laboratoryId/:collection",
    collectionResponse,
  ),

  http.get("*/api/v1/audit-logs", () =>
    HttpResponse.json({ items: [], limit: 20, offset: 0, total: 0 }),
  ),
];
