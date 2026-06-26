import { describe, expect, it } from "vitest";
import {
  assetCategorySchema,
  assetParameterSchema,
  locationSchema,
  optionalText,
  unitSchema,
  userSchema,
} from "./api";

describe("admin api schemas", () => {
  it("parses phone_number from backend users", () => {
    expect(
      userSchema.parse({
        created_at: "2026-06-17T00:00:00Z",
        email: null,
        laboratory: null,
        last_login_at: null,
        phone_number: "13800138000",
        user_id: "00000000-0000-4000-8000-000000000001",
        user_type: {
          name: "root",
          user_type_id: "00000000-0000-4000-8000-000000000002",
        },
        username: "root",
      }).phone_number,
    ).toBe("13800138000");
  });

  it("normalizes optional text payload fields", () => {
    expect(optionalText("  test@example.com ")).toBe("test@example.com");
    expect(optionalText("   ")).toBeNull();
  });

  it("parses asset categories from the backend", () => {
    expect(
      assetCategorySchema.parse({
        category_id: "00000000-0000-4000-8000-000000000031",
        code: "microscope",
        created_at: "2026-06-17T00:00:00Z",
        depth: 0,
        description: "Microscope assets",
        laboratory_id: "00000000-0000-4000-8000-000000000011",
        name: "显微镜",
        parent_category_id: null,
        parameter_assignments: [
          {
            applies_to_descendants: true,
            assignment_id: "00000000-0000-4000-8000-000000000071",
            is_required: true,
            parameter_type_id: "00000000-0000-4000-8000-000000000061",
            sort_order: 1,
          },
        ],
        path: "microscope",
        updated_at: "2026-06-17T00:00:00Z",
      }).parameter_assignments[0].sort_order,
    ).toBe(1);
  });

  it("parses asset parameters from the backend", () => {
    expect(
      assetParameterSchema.parse({
        code: "wavelength_range",
        created_at: "2026-06-17T00:00:00Z",
        data_type: "range",
        default_unit_id: "00000000-0000-4000-8000-000000000051",
        description: "Visible spectrum",
        laboratory_id: "00000000-0000-4000-8000-000000000011",
        name: "波长范围",
        options: [],
        parameter_type_id: "00000000-0000-4000-8000-000000000061",
        unit_dimension: "length",
        updated_at: "2026-06-17T00:00:00Z",
      }).data_type,
    ).toBe("range");
  });

  it("parses locations from the backend", () => {
    expect(
      locationSchema.parse({
        code: "room101",
        created_at: "2026-06-17T00:00:00Z",
        depth: 0,
        description: "Room 101",
        laboratory_id: "00000000-0000-4000-8000-000000000011",
        location_id: "00000000-0000-4000-8000-000000000041",
        name: "101 室",
        parent_location_id: null,
        path: "room101",
        updated_at: "2026-06-17T00:00:00Z",
      }).path,
    ).toBe("room101");
  });

  it("parses units from the backend", () => {
    expect(
      unitSchema.parse({
        allow_decimal: true,
        code: "mm",
        created_at: "2026-06-17T00:00:00Z",
        dimension: "length",
        name: "Millimeter",
        scale_to_base: 0.001,
        symbol: "mm",
        unit_id: "00000000-0000-4000-8000-000000000051",
      }).scale_to_base,
    ).toBe(0.001);
  });
});
