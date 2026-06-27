import { useMutation, useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { userTypeNameSchema } from "../auth/types";
import {
  type LaboratoryDataScope,
  laboratoryCollectionPath,
  laboratoryScopeCacheKey,
  localLaboratoryScope,
} from "../federation/scope";

export const laboratorySchema = z.object({
  laboratory_id: z.string().uuid(),
  name: z.string(),
  address: z.string(),
  description: z.string().nullable(),
  contact: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const laboratoriesSchema = z.array(laboratorySchema);

export const assetCategoryParameterAssignmentSchema = z.object({
  assignment_id: z.string().uuid(),
  parameter_type_id: z.string().uuid(),
  applies_to_descendants: z.boolean(),
  is_required: z.boolean(),
  sort_order: z.number(),
});

export const assetCategorySchema = z.object({
  category_id: z.string().uuid(),
  laboratory_id: z.string().uuid(),
  parent_category_id: z.string().uuid().nullable(),
  name: z.string(),
  code: z.string(),
  path: z.string(),
  depth: z.number(),
  description: z.string().nullable(),
  parameter_assignments: z.array(assetCategoryParameterAssignmentSchema).default([]),
  created_at: z.string(),
  updated_at: z.string(),
});

const assetCategoriesSchema = z.array(assetCategorySchema);

export const assetParameterOptionSchema = z.object({
  option_id: z.string().uuid(),
  parameter_type_id: z.string().uuid(),
  code: z.string(),
  label: z.string(),
  sort_order: z.number(),
});

export const assetParameterSchema = z.object({
  parameter_type_id: z.string().uuid(),
  laboratory_id: z.string().uuid(),
  code: z.string(),
  name: z.string(),
  data_type: z.enum(["text", "number", "range", "boolean", "date", "enum"]),
  unit_dimension: z.string().nullable(),
  default_unit_id: z.string().uuid().nullable(),
  description: z.string().nullable(),
  options: z.array(assetParameterOptionSchema),
  created_at: z.string(),
  updated_at: z.string(),
});

const assetParametersSchema = z.array(assetParameterSchema);

export const locationSchema = z.object({
  location_id: z.string().uuid(),
  laboratory_id: z.string().uuid(),
  parent_location_id: z.string().uuid().nullable(),
  name: z.string(),
  code: z.string(),
  path: z.string(),
  depth: z.number(),
  description: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const locationsSchema = z.array(locationSchema);

export const unitSchema = z.object({
  unit_id: z.string().uuid(),
  code: z.string(),
  name: z.string(),
  symbol: z.string(),
  dimension: z.string(),
  scale_to_base: z.number(),
  allow_decimal: z.boolean(),
  created_at: z.string(),
});

const unitsSchema = z.array(unitSchema);

export const userSchema = z.object({
  user_id: z.string().uuid(),
  username: z.string(),
  email: z.string().nullable(),
  phone_number: z.string().nullable().optional(),
  user_type: z.object({
    user_type_id: z.string().uuid(),
    name: userTypeNameSchema,
  }),
  laboratory: z
    .object({
      laboratory_id: z.string().uuid(),
      name: z.string(),
    })
    .nullable(),
  created_at: z.string(),
  last_login_at: z.string().nullable(),
});

const usersSchema = z.array(userSchema);

export type Laboratory = z.infer<typeof laboratorySchema>;
export type AssetCategory = z.infer<typeof assetCategorySchema>;
export type AssetCategoryParameterAssignment = z.infer<
  typeof assetCategoryParameterAssignmentSchema
>;
export type AssetParameter = z.infer<typeof assetParameterSchema>;
export type AssetParameterOption = z.infer<typeof assetParameterOptionSchema>;
export type Location = z.infer<typeof locationSchema>;
export type Unit = z.infer<typeof unitSchema>;
export type AdminUser = z.infer<typeof userSchema>;

export type LaboratoryPayload = {
  address: string;
  contact: string | null;
  description: string | null;
  name: string;
};

export type AssetCategoryPayload = {
  parent_category_id: string | null;
  name: string;
  code: string;
  description: string | null;
  parameter_assignments?: AssetCategoryParameterAssignmentPayload[];
};

export type AssetCategoryParameterAssignmentPayload = {
  parameter_type_id: string;
  applies_to_descendants: boolean;
  is_required: boolean;
  sort_order: number;
};

export type AssetParameterOptionPayload = {
  option_id?: string;
  code: string;
  label: string;
  sort_order: number;
};

export type AssetParameterPayload = {
  code: string;
  name: string;
  data_type: AssetParameter["data_type"];
  unit_dimension: string | null;
  default_unit_id: string | null;
  description: string | null;
  options: AssetParameterOptionPayload[];
};

export type LocationPayload = {
  parent_location_id: string | null;
  name: string;
  code: string;
  description: string | null;
};

export type UnitPayload = {
  allow_decimal: boolean;
  code: string;
  dimension: string;
  name: string;
  scale_to_base: number;
  symbol: string;
};

export type CreateUserPayload = {
  email: string | null;
  laboratory_id: string | null;
  password: string;
  phone_number: string | null;
  user_type: string;
  username: string;
};

export type UpdateUserPayload = {
  email?: string | null;
  laboratory_id?: string | null;
  phone_number?: string | null;
  user_type?: string;
  username?: string;
};

export const adminQueryKeys = {
  assetCategories: (apiBaseUrl: string, laboratoryId: string) =>
    ["admin", "asset-categories", apiBaseUrl, laboratoryId] as const,
  assetParameters: (apiBaseUrl: string, laboratoryId: string) =>
    ["admin", "asset-parameters", apiBaseUrl, laboratoryId] as const,
  laboratories: (apiBaseUrl: string) => ["admin", "laboratories", apiBaseUrl] as const,
  locations: (apiBaseUrl: string, laboratoryId: string) =>
    ["admin", "locations", apiBaseUrl, laboratoryId] as const,
  units: (apiBaseUrl: string) => ["admin", "units", apiBaseUrl] as const,
  users: (apiBaseUrl: string) => ["admin", "users", apiBaseUrl] as const,
};

export function useLaboratories({ enabled = true }: { enabled?: boolean } = {}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled,
    queryKey: adminQueryKeys.laboratories(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return laboratoriesSchema.parse(await client.get("/laboratories"));
    },
  });
}

export function useUsers() {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    queryKey: adminQueryKeys.users(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return usersSchema.parse(await client.get("/users"));
    },
  });
}

export function useAssetCategories({
  enabled = true,
  laboratoryId,
  scope,
}: {
  enabled?: boolean;
  laboratoryId: string;
  scope?: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const dataScope = scope ?? localLaboratoryScope(laboratoryId);

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: adminQueryKeys.assetCategories(apiBaseUrl, laboratoryScopeCacheKey(dataScope)),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return assetCategoriesSchema.parse(
        await client.get(laboratoryCollectionPath(dataScope, "asset-categories")),
      );
    },
  });
}

export function useAssetParameters({
  enabled = true,
  laboratoryId,
  scope,
}: {
  enabled?: boolean;
  laboratoryId: string;
  scope?: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const dataScope = scope ?? localLaboratoryScope(laboratoryId);

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: adminQueryKeys.assetParameters(apiBaseUrl, laboratoryScopeCacheKey(dataScope)),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return assetParametersSchema.parse(
        await client.get(laboratoryCollectionPath(dataScope, "asset-parameters")),
      );
    },
  });
}

export function useLocations({
  enabled = true,
  laboratoryId,
  scope,
}: {
  enabled?: boolean;
  laboratoryId: string;
  scope?: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const dataScope = scope ?? localLaboratoryScope(laboratoryId);

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: adminQueryKeys.locations(apiBaseUrl, laboratoryScopeCacheKey(dataScope)),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return locationsSchema.parse(
        await client.get(laboratoryCollectionPath(dataScope, "locations")),
      );
    },
  });
}

export function useUnits() {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    queryKey: adminQueryKeys.units(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return unitsSchema.parse(await client.get("/units"));
    },
  });
}

export function useCreateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: LaboratoryPayload) => {
      const client = createApiClient(apiBaseUrl);
      return laboratorySchema.parse(await client.post("/laboratories", payload));
    },
  });
}

export function useUpdateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: Partial<LaboratoryPayload>;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return laboratorySchema.parse(
        await client.patch(`/laboratories/${laboratoryId}`, payload),
      );
    },
  });
}

export function useDeleteLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (laboratoryId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/laboratories/${laboratoryId}`);
    },
  });
}

export function useCreateAssetCategory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: AssetCategoryPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return assetCategorySchema.parse(
        await client.post(`/laboratories/${laboratoryId}/asset-categories`, payload),
      );
    },
  });
}

export function useUpdateAssetCategory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      categoryId,
      payload,
    }: {
      categoryId: string;
      payload: AssetCategoryPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return assetCategorySchema.parse(
        await client.patch(`/asset-categories/${categoryId}`, payload),
      );
    },
  });
}

export function useDeleteAssetCategory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (categoryId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/asset-categories/${categoryId}`);
    },
  });
}

export function useCreateAssetParameter() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: AssetParameterPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return assetParameterSchema.parse(
        await client.post(`/laboratories/${laboratoryId}/asset-parameters`, payload),
      );
    },
  });
}

export function useUpdateAssetParameter() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      parameterId,
      payload,
    }: {
      parameterId: string;
      payload: AssetParameterPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return assetParameterSchema.parse(
        await client.patch(`/asset-parameters/${parameterId}`, payload),
      );
    },
  });
}

export function useDeleteAssetParameter() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (parameterId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/asset-parameters/${parameterId}`);
    },
  });
}

export function useCreateLocation() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: LocationPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return locationSchema.parse(
        await client.post(`/laboratories/${laboratoryId}/locations`, payload),
      );
    },
  });
}

export function useUpdateLocation() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      locationId,
      payload,
    }: {
      locationId: string;
      payload: LocationPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return locationSchema.parse(await client.patch(`/locations/${locationId}`, payload));
    },
  });
}

export function useDeleteLocation() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (locationId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/locations/${locationId}`);
    },
  });
}

export function useCreateUnit() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: UnitPayload) => {
      const client = createApiClient(apiBaseUrl);
      return unitSchema.parse(await client.post("/units", payload));
    },
  });
}

export function useUpdateUnit() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      payload,
      unitId,
    }: {
      payload: Partial<UnitPayload>;
      unitId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return unitSchema.parse(await client.patch(`/units/${unitId}`, payload));
    },
  });
}

export function useDeleteUnit() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (unitId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/units/${unitId}`);
    },
  });
}

export function useCreateUser() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: CreateUserPayload) => {
      const client = createApiClient(apiBaseUrl);
      return userSchema.parse(await client.post("/users", payload));
    },
  });
}

export function useUpdateUser() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      payload,
      userId,
    }: {
      payload: UpdateUserPayload;
      userId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return userSchema.parse(await client.patch(`/users/${userId}`, payload));
    },
  });
}

export function useDeleteUser() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (userId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/users/${userId}`);
    },
  });
}

export function optionalText(value: string | undefined) {
  const normalized = value?.trim() ?? "";
  return normalized.length > 0 ? normalized : null;
}
