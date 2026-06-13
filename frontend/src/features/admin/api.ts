import { useMutation, useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";

const laboratorySchema = z.object({
  laboratory_id: z.string().uuid(),
  name: z.string(),
  address: z.string(),
  description: z.string().nullable(),
  contact: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const laboratoriesSchema = z.array(laboratorySchema);

const userSchema = z.object({
  user_id: z.string().uuid(),
  username: z.string(),
  email: z.string().nullable(),
  user_type: z.object({
    user_type_id: z.string().uuid(),
    name: z.string(),
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

const remoteLaboratorySchema = z.object({
  remote_laboratory_id: z.string().uuid(),
  name: z.string(),
  api_base_url: z.string(),
  is_enabled: z.boolean(),
  key_id: z.string(),
  last_seen_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const remoteLaboratoriesSchema = z.array(remoteLaboratorySchema);

const remoteInventoryItemSchema = z.object({
  inventory_item_id: z.string().uuid(),
  asset_id: z.string().uuid(),
  asset_name: z.string(),
  asset_model: z.string().nullable(),
  laboratory_id: z.string().uuid(),
  laboratory_name: z.string(),
  status: z.string(),
  is_cross_lab_borrowable: z.boolean(),
  quantity_available: z.number(),
  unit_id: z.string().uuid(),
  unit_code: z.string(),
  public_notes: z.string().nullable(),
});

const remoteInventoryResponseSchema = z.object({
  items: z.array(remoteInventoryItemSchema),
});

const remoteBorrowResponseSchema = z.object({
  borrow_request_id: z.string().uuid(),
  correlation_id: z.string().uuid(),
  status: z.string(),
});

export type Laboratory = z.infer<typeof laboratorySchema>;
export type AdminUser = z.infer<typeof userSchema>;
export type RemoteLaboratory = z.infer<typeof remoteLaboratorySchema>;
export type RemoteInventoryItem = z.infer<typeof remoteInventoryItemSchema>;

export type LaboratoryPayload = {
  name: string;
  address: string;
  description: string | null;
  contact: string | null;
};

export type CreateUserPayload = {
  username: string;
  password: string;
  user_type: string;
  laboratory_id: string | null;
  email: string | null;
};

export type UpdateUserPayload = {
  username?: string;
  password?: string;
  user_type?: string;
  laboratory_id?: string | null;
  email?: string | null;
};

export type RemoteLaboratoryPayload = {
  remote_laboratory_id: string;
  name: string;
  api_base_url: string;
  is_enabled: boolean;
  key_id: string;
  shared_secret: string;
};

export type RemoteBorrowPayload = {
  inventory_item_id: string;
  requested_quantity: number;
  purpose: string;
};

export const adminQueryKeys = {
  laboratories: (apiBaseUrl: string) => ["admin", "laboratories", apiBaseUrl] as const,
  remoteInventory: (apiBaseUrl: string, remoteLaboratoryId: string | null) =>
    ["admin", "remote-inventory", apiBaseUrl, remoteLaboratoryId] as const,
  remoteLaboratories: (apiBaseUrl: string) =>
    ["admin", "remote-laboratories", apiBaseUrl] as const,
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

export function useCreateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: LaboratoryPayload) => {
      const client = createApiClient(apiBaseUrl);
      return laboratorySchema.parse(await client.post("/laboratories", payload));
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

export function useUpdateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: LaboratoryPayload;
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

export function useRemoteLaboratories() {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    queryKey: adminQueryKeys.remoteLaboratories(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return remoteLaboratoriesSchema.parse(await client.get("/remote-laboratories"));
    },
  });
}

export function useCreateRemoteLaboratory() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: RemoteLaboratoryPayload) => {
      const client = createApiClient(apiBaseUrl);
      return remoteLaboratorySchema.parse(await client.post("/remote-laboratories", payload));
    },
  });
}

export function useRemoteInventory(remoteLaboratoryId: string | null) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: Boolean(remoteLaboratoryId),
    queryKey: adminQueryKeys.remoteInventory(apiBaseUrl, remoteLaboratoryId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return remoteInventoryResponseSchema.parse(
        await client.get(`/remote-laboratories/${remoteLaboratoryId}/inventory-items`),
      );
    },
  });
}

export function useCreateRemoteBorrowRequest() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      payload,
      remoteLaboratoryId,
    }: {
      payload: RemoteBorrowPayload;
      remoteLaboratoryId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return remoteBorrowResponseSchema.parse(
        await client.post(
          `/remote-laboratories/${remoteLaboratoryId}/borrow-requests`,
          payload,
        ),
      );
    },
  });
}
