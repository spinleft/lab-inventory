import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { queryKeys } from "../../shared/api/queryKeys";

const laboratorySchema = z.object({
  laboratory_id: z.string(),
  name: z.string(),
  address: z.string(),
  description: z.string().nullable(),
  contact: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

const userSchema = z.object({
  user_id: z.string(),
  username: z.string(),
  email: z.string().nullable(),
  user_type: z.object({
    user_type_id: z.string(),
    name: z.string(),
  }),
  laboratory: z
    .object({
      laboratory_id: z.string(),
      name: z.string(),
    })
    .nullable(),
  created_at: z.string(),
  last_login_at: z.string().nullable(),
});

const laboratoriesSchema = z.array(laboratorySchema);
const usersSchema = z.array(userSchema);

export type Laboratory = z.infer<typeof laboratorySchema>;
export type ManagedUser = z.infer<typeof userSchema>;

export type CreateLaboratoryInput = {
  name: string;
  address: string;
  description?: string;
  contact?: string;
};

export type UpdateLaboratoryInput = Partial<CreateLaboratoryInput> & {
  laboratory_id: string;
};

export type CreateUserInput = {
  username: string;
  password: string;
  user_type: string;
  laboratory_id?: string;
  email?: string;
};

export type UpdateUserInput = {
  user_id: string;
  username?: string;
  password?: string;
  user_type?: string;
  laboratory_id?: string | null;
  email?: string;
};

export function useLaboratories(enabled = true) {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.admin.laboratories(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return laboratoriesSchema.parse(await client.get("/laboratories"));
    },
    enabled,
  });
}

export function useUsers() {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.admin.users(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return usersSchema.parse(await client.get("/users"));
    },
  });
}

export function useCreateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: CreateLaboratoryInput) => {
      const client = createApiClient(apiBaseUrl);
      return laboratorySchema.parse(await client.post("/laboratories", input));
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

export function useUpdateLaboratory() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ laboratory_id, ...input }: UpdateLaboratoryInput) => {
      const client = createApiClient(apiBaseUrl);
      return laboratorySchema.parse(
        await client.patch(`/laboratories/${laboratory_id}`, input),
      );
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

export function useDeleteLaboratory() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (laboratoryId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/laboratories/${laboratoryId}`);
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

export function useCreateUser() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: CreateUserInput) => {
      const client = createApiClient(apiBaseUrl);
      return userSchema.parse(await client.post("/users", input));
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

export function useUpdateUser() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ user_id, ...input }: UpdateUserInput) => {
      const client = createApiClient(apiBaseUrl);
      return userSchema.parse(await client.patch(`/users/${user_id}`, input));
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

export function useDeleteUser() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (userId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/users/${userId}`);
    },
    onSuccess: () => invalidateAdminLists(queryClient, apiBaseUrl),
  });
}

function invalidateAdminLists(
  queryClient: ReturnType<typeof useQueryClient>,
  apiBaseUrl: string,
) {
  void queryClient.invalidateQueries({
    queryKey: queryKeys.admin.laboratories(apiBaseUrl),
  });
  void queryClient.invalidateQueries({
    queryKey: queryKeys.admin.users(apiBaseUrl),
  });
}
