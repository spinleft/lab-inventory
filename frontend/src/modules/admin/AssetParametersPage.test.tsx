import { screen, waitFor, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { LAB_CHEMISTRY_ID, testRootUser } from "../../shared/test/fixtures";
import { renderRoute, signIn } from "../../shared/test/render";
import { server } from "../../shared/test/server";

const PARAMETER_ID = "00000000-0000-4000-8000-000000000061";
const VALUES_PATH = `*/api/v1/admin/laboratories/${LAB_CHEMISTRY_ID}/asset-parameters/${PARAMETER_ID}`;

const textParameter = {
  code: "serial_no",
  created_at: "2026-06-17T00:00:00Z",
  data_type: "text",
  default_unit_id: null,
  description: null,
  laboratory_id: LAB_CHEMISTRY_ID,
  name: "序列号",
  options: [],
  parameter_type_id: PARAMETER_ID,
  unit_dimension: null,
  updated_at: "2026-06-17T00:00:00Z",
};

const storedValues = [
  {
    asset_id: "00000000-0000-4000-8000-0000000000c1",
    asset_model: "TDS220",
    asset_name: "示波器",
    data_type: "text",
    value: { text: "SN-1" },
    value_id: "00000000-0000-4000-8000-000000000071",
  },
  {
    asset_id: "00000000-0000-4000-8000-0000000000c2",
    asset_model: null,
    asset_name: "万用表",
    data_type: "text",
    value: { text: "SN-2" },
    value_id: "00000000-0000-4000-8000-000000000072",
  },
];

describe("AssetParametersPage data type changes", () => {
  it("edits one target per stored value and sends the migration with the update", async () => {
    let patched: Record<string, unknown> | null = null;
    server.use(
      http.get(`*/api/v1/admin/laboratories/${LAB_CHEMISTRY_ID}/asset-parameters`, () =>
        HttpResponse.json([textParameter]),
      ),
      http.get(`${VALUES_PATH}/values`, () => HttpResponse.json(storedValues)),
      http.patch(VALUES_PATH, async ({ request }) => {
        patched = (await request.json()) as Record<string, unknown>;
        return HttpResponse.json({
          ...textParameter,
          ...patched,
          updated_at: "2026-06-17T01:00:00Z",
        });
      }),
    );
    signIn(testRootUser);
    const { user } = renderRoute(["/admin/asset-parameters"]);

    const row = (await screen.findByText("序列号")).closest("tr");
    await user.click(within(row as HTMLElement).getByRole("button", { name: "编辑" }));
    await user.click(await screen.findByLabelText("数据类型"));
    await user.click(await screen.findByRole("option", { name: "数字" }));

    expect(await screen.findByRole("heading", { name: "参数值迁移" })).toBeInTheDocument();
    expect(screen.getByText("2 条待迁移")).toBeInTheDocument();
    // Both readings are in the row: what the value says today, and the input for
    // what it should say afterwards.
    expect(screen.getByText("SN-1")).toBeInTheDocument();

    await user.type(screen.getByLabelText("示波器迁移后的值"), "2.5");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() =>
      expect(patched).toMatchObject({
        data_type: "number",
        value_migrations: [
          { value_id: storedValues[0].value_id, value: { number: 2.5 } },
          { value_id: storedValues[1].value_id, value: null },
        ],
      }),
    );
  });
});
