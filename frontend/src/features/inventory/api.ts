import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { paginatedSchema, toSearchParams } from "../../shared/api/pagination";
import { queryKeys } from "../../shared/api/queryKeys";

export type InventoryListParams = {
  q?: string;
  status?: string;
  tracking_mode?: string;
  is_cross_lab_borrowable?: boolean;
  limit: number;
  offset: number;
};

const inventoryItemSchema = z.object({
  inventory_item_id: z.string(),
  asset_id: z.string(),
  asset_name: z.string(),
  asset_model: z.string().nullable(),
  laboratory_id: z.string(),
  laboratory_name: z.string(),
  tracking_mode: z.string(),
  serial_number: z.string().nullable(),
  batch_number: z.string().nullable(),
  quantity_on_hand: z.number(),
  quantity_allocated: z.number(),
  quantity_available: z.number(),
  unit_code: z.string(),
  location_name: z.string().nullable(),
  status: z.string(),
  is_cross_lab_borrowable: z.boolean(),
  public_notes: z.string().nullable(),
  internal_notes: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const inventoryListSchema = paginatedSchema(inventoryItemSchema);

export type InventoryItem = z.infer<typeof inventoryItemSchema>;

export function useInventoryItems(params: InventoryListParams) {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.inventory.list(apiBaseUrl, params),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      const query = toSearchParams(params);
      return inventoryListSchema.parse(await client.get(`/inventory-items${query}`));
    },
  });
}
