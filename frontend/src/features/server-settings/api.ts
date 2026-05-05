import { useMutation } from "@tanstack/react-query";
import { z } from "zod";
import { createApiClient } from "../../shared/api/httpClient";

const healthCheckSchema = z.object({
  status: z.string(),
});

export function useTestBackendConnection() {
  return useMutation({
    mutationFn: async (apiBaseUrl: string) => {
      const client = createApiClient(apiBaseUrl);
      return healthCheckSchema.parse(await client.get("/health_check"));
    },
  });
}
