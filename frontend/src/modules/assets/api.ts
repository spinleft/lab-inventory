import { useMutation, useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { type AttachmentClaim } from "../attachments/api";
import { isSystemAdmin } from "../auth/permissions";
import {
  type LaboratoryDataScope,
  assetDetailPath,
  laboratoryCollectionPath,
  laboratoryDetailScopeCacheKey,
  laboratoryScopeCacheKey,
  localLaboratoryPath,
  localLaboratoryScope,
} from "../federation/scope";

export const assetTrackingModeSchema = z.enum(["serialized", "quantity"]);
export const assetParameterDataTypeSchema = z.enum([
  "text",
  "number",
  "range",
  "boolean",
  "date",
  "enum",
]);

const assetInventorySummarySchema = z.object({
  item_count: z.number(),
  quantity_allocated: z.number(),
  quantity_on_hand: z.number(),
});

export const assetInventoryItemSchema = z.object({
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
  serial_number: z.string().nullable(),
  status: z.string(),
  tracking_mode: assetTrackingModeSchema,
  updated_at: z.string(),
});

const assetParameterRuntimeValueSchema = z
  .object({
    boolean: z.boolean().nullable().optional(),
    date: z.string().nullable().optional(),
    number: z.number().nullable().optional(),
    number_in_base: z.number().nullable().optional(),
    option_code: z.string().nullable().optional(),
    option_id: z.string().uuid().nullable().optional(),
    option_label: z.string().nullable().optional(),
    range_end: z.number().nullable().optional(),
    range_end_in_base: z.number().nullable().optional(),
    range_start: z.number().nullable().optional(),
    range_start_in_base: z.number().nullable().optional(),
    text: z.string().nullable().optional(),
    unit_id: z.string().uuid().nullable().optional(),
  })
  .catchall(z.unknown());

export const assetParameterValueSchema = z.object({
  asset_id: z.string().uuid(),
  code: z.string(),
  created_at: z.string(),
  data_type: assetParameterDataTypeSchema,
  default_unit_id: z.string().uuid().nullable(),
  laboratory_id: z.string().uuid(),
  name: z.string(),
  parameter_type_id: z.string().uuid(),
  unit_dimension: z.string().nullable(),
  updated_at: z.string(),
  value: assetParameterRuntimeValueSchema,
  value_id: z.string().uuid(),
});

export const assetSchema = z.object({
  asset_id: z.string().uuid(),
  category_id: z.string().uuid().nullable(),
  created_at: z.string(),
  internal_notes: z.string().nullable(),
  inventory_items: z.array(assetInventoryItemSchema).optional(),
  inventory_unit_id: z.string().uuid(),
  inventory_summary: assetInventorySummarySchema,
  laboratory_id: z.string().uuid(),
  manufacturer: z.string().nullable(),
  model: z.string().nullable(),
  name: z.string(),
  parameters: z.array(assetParameterValueSchema).optional(),
  public_notes: z.string().nullable(),
  tracking_mode: assetTrackingModeSchema,
  updated_at: z.string(),
});

const assetsResponseSchema = z.object({
  items: z.array(assetSchema),
  limit: z.number(),
  offset: z.number(),
  total: z.number(),
});

export type AssetTrackingMode = z.infer<typeof assetTrackingModeSchema>;
export type AssetParameterDataType = z.infer<typeof assetParameterDataTypeSchema>;
export type AssetParameterRuntimeValue = z.infer<typeof assetParameterRuntimeValueSchema>;
export type AssetParameterValue = z.infer<typeof assetParameterValueSchema>;
export type AssetInventoryItem = z.infer<typeof assetInventoryItemSchema>;
export type Asset = z.infer<typeof assetSchema>;
export type AssetsResponse = z.infer<typeof assetsResponseSchema>;

export type AssetQuery = {
  category_id?: string;
  exact_category?: boolean;
  has_inventory?: boolean;
  include?: "parameters";
  inventory_status?: string;
  keyword?: string;
  limit: number;
  location_id?: string;
  manufacturer?: string;
  offset: number;
  parameter_filters?: string;
  tracking_mode?: AssetTrackingMode;
};

export type AssetInventoryItemPayload = {
  attachments?: AttachmentClaim[];
  batch_number?: string | null;
  internal_notes?: string | null;
  location_id?: string | null;
  public_notes?: string | null;
  quantity_allocated?: number | null;
  quantity_on_hand?: number | null;
  serial_number?: string | null;
  status?: string | null;
};

export type AssetParameterPayloadValue =
  | string
  | number
  | boolean
  | {
      boolean?: boolean;
      date?: string;
      number?: number;
      option_id?: string;
      range_end?: number;
      range_start?: number;
      text?: string;
      unit_id?: string | null;
    };

export type AssetParameterValuePayload = {
  parameter_type_id: string;
  value: AssetParameterPayloadValue | null;
};

export type AssetPayload = {
  attachments?: AttachmentClaim[];
  category_id?: string | null;
  internal_notes?: string | null;
  inventory_items?: AssetInventoryItemPayload[];
  inventory_unit_id?: string;
  manufacturer?: string | null;
  model?: string | null;
  name?: string;
  parameters?: AssetParameterValuePayload[];
  public_notes?: string | null;
  tracking_mode?: AssetTrackingMode;
};

export const assetQueryKeys = {
  detail: (
    apiBaseUrl: string,
    scopeKey: string,
    assetId: string,
    includeParameters: boolean,
  ) => ["assets", apiBaseUrl, "detail", scopeKey, assetId, includeParameters] as const,
  list: (apiBaseUrl: string, scopeKey: string, query: AssetQuery) =>
    ["assets", apiBaseUrl, "list", scopeKey, query] as const,
  root: (apiBaseUrl: string) => ["assets", apiBaseUrl] as const,
};

export function useAssets({
  enabled = true,
  laboratoryId,
  query,
  scope,
}: {
  enabled?: boolean;
  laboratoryId: string;
  query: AssetQuery;
  scope?: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();
  const dataScope = scope ?? localLaboratoryScope(laboratoryId);

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: assetQueryKeys.list(apiBaseUrl, laboratoryScopeCacheKey(dataScope), query),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return assetsResponseSchema.parse(
        await client.get(
          laboratoryCollectionPath(dataScope, "assets", isSystemAdmin(currentUser)),
          query,
        ),
      );
    },
  });
}

export function useAsset({
  assetId,
  enabled = true,
  includeParameters = true,
  scope,
}: {
  assetId: string;
  enabled?: boolean;
  includeParameters?: boolean;
  scope?: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();
  const scopeKey = laboratoryDetailScopeCacheKey(scope);

  return useQuery({
    enabled: enabled && Boolean(assetId),
    queryKey: assetQueryKeys.detail(apiBaseUrl, scopeKey, assetId, includeParameters),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return assetSchema.parse(
        await client.get(
          assetDetailPath(scope, assetId, isSystemAdmin(currentUser)),
          includeParameters ? { include: "parameters" } : undefined,
        ),
      );
    },
  });
}

export function useCreateAsset() {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: Required<Pick<AssetPayload, "inventory_unit_id" | "name" | "tracking_mode">> &
        AssetPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return assetSchema.parse(
        await client.post(
          localLaboratoryPath(laboratoryId, "assets", isSystemAdmin(currentUser)),
          payload,
        ),
      );
    },
  });
}

export function useUpdateAsset() {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useMutation({
    mutationFn: async ({ assetId, laboratoryId, payload }: { assetId: string; laboratoryId: string; payload: AssetPayload }) => {
      const client = createApiClient(apiBaseUrl);
      return assetSchema.parse(
        await client.patch(
          localLaboratoryPath(laboratoryId, `assets/${assetId}`, isSystemAdmin(currentUser)),
          payload,
        ),
      );
    },
  });
}

export function useDeleteAsset() {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useMutation({
    mutationFn: async ({ assetId, laboratoryId }: { assetId: string; laboratoryId: string }) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(
        localLaboratoryPath(laboratoryId, `assets/${assetId}`, isSystemAdmin(currentUser)),
      );
    },
  });
}
