import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import {
  type LaboratoryDataScope,
  laboratoryCollectionPath,
  laboratoryDetailScopeCacheKey,
  laboratoryScopeKey,
} from "../federation/scope";
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
  status: z.enum(["pending", "approved", "rejected", "cancelled"]),
  updated_at: z.string(),
});

const borrowRequestsSchema = z.array(borrowRequestSchema);

/**
 * A request as its own requester sees it, which is the narrower shape both
 * `/local/borrow-requests/mine` and the federation read answer with. It carries
 * neither the reviewer's identity nor the lending laboratory's internal
 * identifiers for the requester.
 */
const myBorrowRequestSchema = z.object({
  asset_model: z.string().nullable(),
  asset_name: z.string(),
  borrow_request_id: z.string().uuid(),
  created_at: z.string(),
  decision_note: z.string().nullable(),
  inventory_item_id: z.string().uuid(),
  inventory_status: z.string(),
  laboratory_id: z.string().uuid(),
  request_note: z.string().nullable(),
  reviewed_at: z.string().nullable(),
  status: z.enum(["pending", "approved", "rejected", "cancelled"]),
  updated_at: z.string(),
});

const myBorrowRequestsSchema = z.array(myBorrowRequestSchema);

export type BorrowRequest = z.infer<typeof borrowRequestSchema>;
export type BorrowRequestStatus = BorrowRequest["status"];
export type MyBorrowRequest = z.infer<typeof myBorrowRequestSchema>;

/** A request paired with the laboratory it was filed at, so an action knows where to send. */
export type ScopedBorrowRequest = MyBorrowRequest & {
  laboratoryName: string;
  scope: LaboratoryDataScope;
};

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
  mine: (apiBaseUrl: string, scope: LaboratoryDataScope) =>
    ["borrow-requests", apiBaseUrl, "mine", laboratoryScopeKey(scope)] as const,
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

/**
 * Files a borrow request against the selected laboratory, local or remote.
 *
 * The remote path goes out through the federation proxy, which relays the
 * lending laboratory's own answer — including its conflicts — back unchanged.
 * `systemAdmin` is deliberately never passed: there is no admin-scoped borrow
 * route, and a system administrator cannot borrow in any case.
 */
export function useCreateBorrowRequest() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      inventoryItemId,
      payload,
      scope,
    }: {
      inventoryItemId: string;
      payload: CreateBorrowRequestPayload;
      scope: LaboratoryDataScope;
    }) => {
      const client = createApiClient(apiBaseUrl);
      const path = laboratoryCollectionPath(
        scope,
        `inventory-items/${inventoryItemId}/borrow-requests`,
      );
      return myBorrowRequestSchema.parse(await client.post(path, payload));
    },
    onSuccess: (_request, variables) => {
      queryClient.invalidateQueries({ queryKey: borrowRequestQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({
        queryKey: [
          "inventory-items",
          apiBaseUrl,
          "detail",
          laboratoryDetailScopeCacheKey(variables.scope),
          variables.inventoryItemId,
        ],
      });
    },
  });
}

/** Retracts a request the current user filed, at whichever laboratory holds it. */
export function useCancelBorrowRequest() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      borrowRequestId,
      scope,
    }: {
      borrowRequestId: string;
      scope: LaboratoryDataScope;
    }) => {
      const client = createApiClient(apiBaseUrl);
      const path = laboratoryCollectionPath(scope, `borrow-requests/${borrowRequestId}/cancel`);
      return myBorrowRequestSchema.parse(await client.post(path));
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: borrowRequestQueryKeys.root(apiBaseUrl) });
      queryClient.invalidateQueries({ queryKey: inventoryQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useMyBorrowRequests({
  enabled = true,
  scope,
}: {
  enabled?: boolean;
  scope: LaboratoryDataScope;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled,
    queryKey: borrowRequestQueryKeys.mine(apiBaseUrl, scope),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return myBorrowRequestsSchema.parse(await client.get(myBorrowRequestsPath(scope)));
    },
  });
}

/**
 * The current user's requests across every laboratory they can reach.
 *
 * A request lives only on the instance that owns the item, so there is no single
 * endpoint to ask — fanning out over the local laboratory plus each active trust
 * is the shape of the data, not a workaround for a missing route. Each row
 * carries the scope it came from so the cancel action knows where to send.
 */
export function useMyBorrowRequestsAcrossScopes(
  scopes: { name: string; scope: LaboratoryDataScope }[],
) {
  const { apiBaseUrl } = useBackendConfig();

  return useQueries({
    queries: scopes.map(({ name, scope }) => ({
      queryKey: borrowRequestQueryKeys.mine(apiBaseUrl, scope),
      queryFn: async (): Promise<ScopedBorrowRequest[]> => {
        const client = createApiClient(apiBaseUrl);
        const requests = myBorrowRequestsSchema.parse(
          await client.get(myBorrowRequestsPath(scope)),
        );
        return requests.map((request) => ({ ...request, laboratoryName: name, scope }));
      },
    })),
    combine: (results) => ({
      // One unreachable partner must not blank the whole page, so failures are
      // surfaced as a count alongside whatever did load.
      failureCount: results.filter((result) => result.isError).length,
      isFetching: results.some((result) => result.isFetching),
      isLoading: results.some((result) => result.isLoading),
      refetch: () => results.forEach((result) => result.refetch()),
      requests: results
        .flatMap((result) => result.data ?? [])
        .sort((left, right) => right.created_at.localeCompare(left.created_at)),
    }),
  });
}

/**
 * The local route is `/local/borrow-requests/mine`, but a federated caller is
 * already identified by the signature on the request, so its whole borrow list
 * is only ever their own and needs no `mine` segment.
 */
function myBorrowRequestsPath(scope: LaboratoryDataScope) {
  return scope.kind === "remote"
    ? laboratoryCollectionPath(scope, "borrow-requests")
    : laboratoryCollectionPath(scope, "borrow-requests/mine");
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
