import { http, HttpResponse } from "msw";

export const testUser = {
  user_id: "00000000-0000-0000-0000-000000000001",
  username: "admin",
  email: "admin@example.com",
  user_type: {
    user_type_id: "00000000-0000-0000-0000-000000000002",
    name: "owner",
  },
  laboratory: null,
};

export const handlers = [
  http.get("*/api/v1/health_check", () => HttpResponse.json({ status: "ok" })),
  http.post("*/api/v1/auth/login", () =>
    HttpResponse.json({ message: "Login successful" }),
  ),
  http.post("*/api/v1/auth/logout", () =>
    HttpResponse.json({ message: "Logout successful" }),
  ),
  http.patch("*/api/v1/auth/password", () =>
    HttpResponse.json({ message: "Password changed" }),
  ),
  http.get("*/api/v1/auth/me", () => HttpResponse.json(testUser)),
  http.get("*/api/v1/stock-alerts", () => HttpResponse.json([])),
  http.get("*/api/v1/borrow-request-alerts", () => HttpResponse.json([])),
  http.get("*/api/v1/maintenance-alerts", () => HttpResponse.json([])),
  http.get("*/api/v1/assets", () =>
    HttpResponse.json({
      items: [
        {
          asset_id: "10000000-0000-0000-0000-000000000001",
          laboratory_id: "20000000-0000-0000-0000-000000000001",
          laboratory_name: "材料实验室",
          category_id: null,
          category_name: "电子仪器",
          asset_kind: "device",
          tracking_mode: "unique",
          name: "示波器",
          model: "DS1054Z",
          manufacturer: "Rigol",
          default_unit_id: "30000000-0000-0000-0000-000000000001",
          default_unit_code: "台",
          minimum_stock_quantity: null,
          minimum_stock_unit_id: null,
          minimum_stock_unit_code: null,
          public_notes: null,
          internal_notes: null,
          is_archived: false,
          created_at: "2026-05-01T00:00:00Z",
          updated_at: "2026-05-01T00:00:00Z",
        },
      ],
      limit: 20,
      offset: 0,
      total: 1,
    }),
  ),
  http.get("*/api/v1/inventory-items", () =>
    HttpResponse.json({
      items: [
        {
          inventory_item_id: "40000000-0000-0000-0000-000000000001",
          asset_id: "10000000-0000-0000-0000-000000000001",
          asset_name: "示波器",
          asset_model: "DS1054Z",
          laboratory_id: "20000000-0000-0000-0000-000000000001",
          laboratory_name: "材料实验室",
          tracking_mode: "unique",
          serial_number: "SN-001",
          batch_number: null,
          quantity_on_hand: 1,
          quantity_allocated: 0,
          quantity_available: 1,
          unit_id: "30000000-0000-0000-0000-000000000001",
          unit_code: "台",
          location_id: null,
          location_name: "A-101",
          status: "idle",
          is_cross_lab_borrowable: true,
          public_notes: null,
          internal_notes: null,
          created_at: "2026-05-01T00:00:00Z",
          updated_at: "2026-05-01T00:00:00Z",
        },
      ],
      limit: 20,
      offset: 0,
      total: 1,
    }),
  ),
];
