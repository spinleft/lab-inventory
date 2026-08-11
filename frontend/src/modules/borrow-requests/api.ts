import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { inventoryItemSchema, inventoryQueryKeys } from "../inventory/api";

const borrowRequestSchema = z.object({
  asset_model: z.string().nullable(),
  asset_name: z.string(),
  borrow_request_id: z.string().uuid(),
  created_at: z.string(),
  decision_note: z.string().nullable(),
  inventory_item_id: z.string().uuid(),
  inventory_item_title: z.string(),
  inventory_status: z.string(),
  local_laboratory_id: z.string().uuid(),
  requester_guest_link_id: z.string().uuid().nullable(),
  requester_user_id: z.string().uuid().nullable(),
  requester_user_type: z.string(),
  requester_username: z.string(),
  request_note: z.string().nullable(),
  reviewed_at: z.string().nullable(),
  reviewed_by_user_id: z.string().uuid().nullable(),
  reviewed_by_user_type: z.string().nullable(),
  reviewed_by_username: z.string().nullable(),
  status: z.enum(["pending", "approved", "rejected"]),
  updated_at: z.string(),
});

const borrowRequestsSchema = z.array(borrowRequestSchema);

export type BorrowRequest = z.infer<typeof borrowRequestSchema>;
export type BorrowRequestStatus = BorrowRequest["status"];

export type CreateBorrowRequestPayload = {
  request_note?: string;
};

export type ResolveBorrowRequestPayload = {
  decision: "approved" | "rejected";
  decision_note?: string;
};

export const borrowRequestQueryKeys = {
  list: (apiBaseUrl: string, laboratoryId: string, status?: BorrowRequestStatus | "all") =>
    ["borrow-requests", apiBaseUrl, laboratoryId, status ?? "all"] as const,
  root: (apiBaseUrl: string) => ["borrow-requests", apiBaseUrl] as const,
};

export function useBorrowRequests({
  enabled = true,
  laboratoryId,
  status,
}: {
  enabled?: boolean;
  laboratoryId: string;
  status?: BorrowRequestStatus | "all";
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: borrowRequestQueryKeys.list(apiBaseUrl, laboratoryId, status),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return borrowRequestsSchema.parse(
        await client.get("/local/borrow-requests",
          status && status !== "all" ? { status } : undefined,
        ),
      );
    },
  });
}

export function useResolveBorrowRequest() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      borrowRequestId,
      payload,
    }: {
      laboratoryId: string;
      borrowRequestId: string;
      payload: ResolveBorrowRequestPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return borrowRequestSchema.parse(
        await client.patch(`/local/borrow-requests/${borrowRequestId}`, payload),
      );
    },
    onSuccess: (_request, variables) => {
      queryClient.invalidateQueries({ queryKey: borrowRequestQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({
        queryKey: borrowRequestQueryKeys.list(apiBaseUrl, variables.laboratoryId, "pending"),
      });
    },
  });
}

export function useCreateBorrowRequest() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      inventoryItemId,
      payload,
    }: {
      inventoryItemId: string;
      payload: CreateBorrowRequestPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return borrowRequestSchema.parse(
        await client.post(`/local/inventory-items/${inventoryItemId}/borrow-requests`, payload),
      );
    },
    onSuccess: (_request, variables) => {
      queryClient.invalidateQueries({ queryKey: borrowRequestQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({
        queryKey: ["inventory-items", apiBaseUrl, "detail", "local", variables.inventoryItemId],
      });
    },
  });
}

export function useBorrowedInventoryItems(laboratoryId: string) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: Boolean(laboratoryId),
    queryKey: ["inventory-items", apiBaseUrl, "borrowed", laboratoryId] as const,
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return z
        .object({
          items: z.array(inventoryItemSchema),
          limit: z.number(),
          offset: z.number(),
          total: z.number(),
        })
        .parse(
          await client.get("/local/inventory-items", {
            status: "borrowed",
            limit: 200,
            offset: 0,
          }),
        );
    },
  });
}
