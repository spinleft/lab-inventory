import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { type AttachmentClaim } from "../attachments/api";
import {
  assetQueryKeys,
  assetTrackingModeSchema,
  type AssetTrackingMode,
} from "../assets/api";

export const inventoryStatusSchema = z.enum([
  "available",
  "reserved",
  "retired",
  "lost",
  "consumed",
]);

const inventoryItemAssetSchema = z.object({
  asset_id: z.string().uuid(),
  category_id: z.string().uuid().nullable(),
  default_unit_id: z.string().uuid(),
  manufacturer: z.string().nullable(),
  model: z.string().nullable(),
  name: z.string(),
});

export const inventoryItemSchema = z.object({
  asset: inventoryItemAssetSchema,
  asset_id: z.string().uuid(),
  batch_number: z.string().nullable(),
  created_at: z.string(),
  internal_notes: z.string().nullable(),
  inventory_item_id: z.string().uuid(),
  laboratory_id: z.string().uuid(),
  last_stocktake_at: z.string().nullable(),
  location_id: z.string().uuid().nullable(),
  public_notes: z.string().nullable(),
  quantity_allocated: z.number(),
  quantity_on_hand: z.number(),
  quantity_unit_id: z.string().uuid(),
  serial_number: z.string().nullable(),
  status: inventoryStatusSchema,
  tracking_mode: assetTrackingModeSchema,
  updated_at: z.string(),
});

const inventoryItemsResponseSchema = z.object({
  items: z.array(inventoryItemSchema),
  limit: z.number(),
  offset: z.number(),
  total: z.number(),
});

export type InventoryStatus = z.infer<typeof inventoryStatusSchema>;
export type InventoryItemAsset = z.infer<typeof inventoryItemAssetSchema>;
export type InventoryItem = z.infer<typeof inventoryItemSchema>;
export type InventoryItemsResponse = z.infer<typeof inventoryItemsResponseSchema>;

export type InventoryItemQuery = {
  asset_id?: string;
  batch_number?: string;
  category_id?: string;
  exact_category?: boolean;
  has_batch?: boolean;
  has_location?: boolean;
  keyword?: string;
  limit: number;
  location_id?: string;
  offset: number;
  serial_number?: string;
  status?: InventoryStatus;
  tracking_mode?: AssetTrackingMode;
};

export type CreateInventoryItemsPayload = {
  attachments?: AttachmentClaim[];
  batch_number?: string | null;
  count?: number;
  internal_notes?: string | null;
  location_id?: string | null;
  public_notes?: string | null;
  quantity_allocated?: number;
  quantity_on_hand?: number;
  serial_numbers?: string[];
  status?: InventoryStatus;
};

export type UpdateInventoryItemPayload = {
  batch_number?: string | null;
  internal_notes?: string | null;
  location_id?: string | null;
  public_notes?: string | null;
  quantity_allocated?: number;
  quantity_on_hand?: number;
  serial_number?: string;
  status?: InventoryStatus;
};

export const inventoryQueryKeys = {
  detail: (apiBaseUrl: string, inventoryItemId: string) =>
    ["inventory-items", apiBaseUrl, "detail", inventoryItemId] as const,
  list: (apiBaseUrl: string, laboratoryId: string, query: InventoryItemQuery) =>
    ["inventory-items", apiBaseUrl, "list", laboratoryId, query] as const,
  root: (apiBaseUrl: string) => ["inventory-items", apiBaseUrl] as const,
};

export function useInventoryItems({
  enabled = true,
  laboratoryId,
  query,
}: {
  enabled?: boolean;
  laboratoryId: string;
  query: InventoryItemQuery;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: inventoryQueryKeys.list(apiBaseUrl, laboratoryId, query),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return inventoryItemsResponseSchema.parse(
        await client.get(`/laboratories/${laboratoryId}/inventory-items`, query),
      );
    },
  });
}

export function useInventoryItem({
  enabled = true,
  inventoryItemId,
}: {
  enabled?: boolean;
  inventoryItemId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(inventoryItemId),
    queryKey: inventoryQueryKeys.detail(apiBaseUrl, inventoryItemId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return inventoryItemSchema.parse(
        await client.get(`/inventory-items/${inventoryItemId}`),
      );
    },
  });
}

export function useCreateInventoryItems() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      assetId,
      payload,
    }: {
      assetId: string;
      payload: CreateInventoryItemsPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return z
        .array(inventoryItemSchema)
        .parse(await client.post(`/assets/${assetId}/inventory-items`, payload));
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useUpdateInventoryItem() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      inventoryItemId,
      payload,
    }: {
      inventoryItemId: string;
      payload: UpdateInventoryItemPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return inventoryItemSchema.parse(
        await client.patch(`/inventory-items/${inventoryItemId}`, payload),
      );
    },
    onSuccess: (item) => {
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({
        queryKey: inventoryQueryKeys.detail(apiBaseUrl, item.inventory_item_id),
      });
      queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useDeleteInventoryItem() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (inventoryItemId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/inventory-items/${inventoryItemId}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
    },
  });
}
