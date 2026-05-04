import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { paginatedSchema, toSearchParams } from "../../shared/api/pagination";
import { queryKeys } from "../../shared/api/queryKeys";

export type AssetListParams = {
  q?: string;
  asset_kind?: string;
  tracking_mode?: string;
  limit: number;
  offset: number;
};

const assetSchema = z.object({
  asset_id: z.string(),
  laboratory_id: z.string(),
  laboratory_name: z.string(),
  category_name: z.string().nullable(),
  asset_kind: z.string(),
  tracking_mode: z.string(),
  name: z.string(),
  model: z.string().nullable(),
  manufacturer: z.string().nullable(),
  default_unit_code: z.string(),
  minimum_stock_quantity: z.number().nullable(),
  public_notes: z.string().nullable(),
  internal_notes: z.string().nullable(),
  is_archived: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});

const assetListSchema = paginatedSchema(assetSchema);

export type Asset = z.infer<typeof assetSchema>;

export function useAssets(params: AssetListParams) {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.assets.list(apiBaseUrl, params),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      const query = toSearchParams(params);
      return assetListSchema.parse(await client.get(`/assets${query}`));
    },
  });
}
