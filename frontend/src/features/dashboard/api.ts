import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { queryKeys } from "../../shared/api/queryKeys";

const stockAlertSchema = z.object({
  asset_id: z.string(),
  laboratory_name: z.string(),
  name: z.string(),
  model: z.string().nullable(),
  minimum_stock_quantity: z.number().nullable(),
  quantity_available: z.number(),
});

const borrowRequestAlertSchema = z.object({
  borrow_request_id: z.string(),
  alert_kind: z.string(),
  asset_name: z.string(),
  requester_laboratory_name: z.string(),
  owner_laboratory_name: z.string(),
  status: z.string(),
});

const maintenanceAlertSchema = z.object({
  maintenance_schedule_id: z.string(),
  alert_kind: z.string(),
  asset_name: z.string(),
  laboratory_name: z.string(),
  schedule_name: z.string(),
  next_maintenance_at: z.string(),
});

export function useStockAlerts() {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.alerts.stock(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return z.array(stockAlertSchema).parse(await client.get("/stock-alerts"));
    },
  });
}

export function useBorrowRequestAlerts() {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.alerts.borrowRequests(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return z
        .array(borrowRequestAlertSchema)
        .parse(await client.get("/borrow-request-alerts"));
    },
  });
}

export function useMaintenanceAlerts() {
  const { apiBaseUrl } = useBackendConfig();
  return useQuery({
    queryKey: queryKeys.alerts.maintenance(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return z
        .array(maintenanceAlertSchema)
        .parse(await client.get("/maintenance-alerts"));
    },
  });
}
