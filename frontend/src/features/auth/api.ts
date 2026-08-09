import { useMutation, useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { currentUserSchema } from "./types";

type LoginCredentials = {
  password: string;
  username: string;
};

type ChangePasswordInput = {
  current_password: string;
  new_password: string;
  new_password_check: string;
};

const loginResponseSchema = z.object({
  message: z.string(),
});

const messageResponseSchema = z.object({
  message: z.string(),
});

export function useCurrentUser({ enabled = true }: { enabled?: boolean } = {}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled,
    queryKey: ["auth", "me", apiBaseUrl],
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return currentUserSchema.parse(await client.get("/auth/me"));
    },
  });
}

export function useLogin() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (credentials: LoginCredentials) => {
      const client = createApiClient(apiBaseUrl);
      return loginResponseSchema.parse(await client.post("/auth/login", credentials));
    },
  });
}

export function useLogout() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return messageResponseSchema.parse(await client.post("/auth/logout"));
    },
  });
}

export function useChangePassword() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (payload: ChangePasswordInput) => {
      const client = createApiClient(apiBaseUrl);
      return messageResponseSchema.parse(await client.patch("/auth/password", payload));
    },
  });
}
