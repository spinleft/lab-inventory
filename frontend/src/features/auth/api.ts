import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { queryKeys } from "../../shared/api/queryKeys";
import { currentUserSchema } from "./types";

type LoginInput = {
  username: string;
  password: string;
};

export type ChangePasswordInput = {
  current_password: string;
  new_password: string;
  new_password_check: string;
};

const messageResponseSchema = {
  parse: (value: unknown) => value as { message: string },
};

export function useCurrentUser() {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.auth.me(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return currentUserSchema.parse(await client.get("/auth/me"));
    },
  });
}

export function useLogin() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: LoginInput) => {
      const client = createApiClient(apiBaseUrl);
      return messageResponseSchema.parse(await client.post("/auth/login", input));
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.auth.me(apiBaseUrl),
      });
    },
  });
}

export function useLogout() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return messageResponseSchema.parse(await client.post("/auth/logout"));
    },
    onSettled: () => {
      queryClient.clear();
    },
  });
}

export function useChangePassword() {
  const { apiBaseUrl } = useBackendConfig();
  return useMutation({
    mutationFn: async (input: ChangePasswordInput) => {
      const client = createApiClient(apiBaseUrl);
      return messageResponseSchema.parse(await client.patch("/auth/password", input));
    },
  });
}

export function useTestBackendConnection() {
  return useMutation({
    mutationFn: async (apiBaseUrl: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.get("/health_check");
      return true;
    },
  });
}
