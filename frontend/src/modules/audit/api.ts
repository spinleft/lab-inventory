import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";

export const auditLogSchema = z.object({
  audit_log_id: z.string().uuid(),
  actor_user_id: z.string().uuid().nullable(),
  actor_username: z.string().nullable(),
  action: z.string(),
  resource_type: z.string(),
  resource_id: z.string().uuid().nullable(),
  details: z.unknown(),
  created_at: z.string(),
});

const auditLogsResponseSchema = z.object({
  items: z.array(auditLogSchema),
  limit: z.number(),
  offset: z.number(),
  total: z.number(),
});

export type AuditLog = z.infer<typeof auditLogSchema>;

export type AuditLogQuery = {
  action?: string;
  actor_user_id?: string;
  created_from?: string;
  created_to?: string;
  limit: number;
  offset: number;
  resource_id?: string;
  resource_type?: string;
};

export const auditQueryKeys = {
  logs: (apiBaseUrl: string, query: AuditLogQuery) => ["audit", "logs", apiBaseUrl, query] as const,
};

export function useAuditLogs(query: AuditLogQuery) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    queryKey: auditQueryKeys.logs(apiBaseUrl, query),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return auditLogsResponseSchema.parse(await client.get("/audit-logs", query));
    },
  });
}
