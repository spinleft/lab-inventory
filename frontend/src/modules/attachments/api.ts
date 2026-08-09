import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";

export const attachmentVisibilitySchema = z.enum(["public", "internal"]);

export const fileUploadSchema = z.object({
  created_at: z.string(),
  expires_at: z.string(),
  file_size_bytes: z.number(),
  laboratory_id: z.string().uuid(),
  mime_type: z.string().nullable(),
  original_file_name: z.string(),
  sha256_hex: z.string(),
  upload_id: z.string().uuid(),
});

export const attachmentFileSchema = z.object({
  created_at: z.string(),
  file_id: z.string().uuid(),
  file_size_bytes: z.number(),
  mime_type: z.string().nullable(),
  original_file_name: z.string(),
  sha256_hex: z.string(),
  uploaded_by_user_id: z.string().uuid().nullable(),
});

export const attachmentTargetSchema = z.discriminatedUnion("type", [
  z.object({ id: z.string().uuid(), type: z.literal("asset") }),
  z.object({ id: z.string().uuid(), type: z.literal("inventory_item") }),
]);

export const attachmentSchema = z.object({
  attachment_id: z.string().uuid(),
  created_at: z.string(),
  description: z.string().nullable(),
  display_name: z.string(),
  file: attachmentFileSchema,
  file_id: z.string().uuid(),
  laboratory_id: z.string().uuid(),
  target: attachmentTargetSchema,
  updated_at: z.string(),
  visibility: attachmentVisibilitySchema,
});

const attachmentListSchema = z.array(attachmentSchema);

export type AttachmentVisibility = z.infer<typeof attachmentVisibilitySchema>;
export type FileUpload = z.infer<typeof fileUploadSchema>;
export type Attachment = z.infer<typeof attachmentSchema>;

export type AttachmentClaim = {
  description?: string | null;
  display_name?: string | null;
  upload_id: string;
  visibility: AttachmentVisibility;
};

export const attachmentQueryKeys = {
  asset: (apiBaseUrl: string, assetId: string) =>
    ["attachments", apiBaseUrl, "asset", assetId] as const,
  inventoryItem: (apiBaseUrl: string, inventoryItemId: string) =>
    ["attachments", apiBaseUrl, "inventory-item", inventoryItemId] as const,
  root: (apiBaseUrl: string) => ["attachments", apiBaseUrl] as const,
};

export function useAssetAttachments({
  assetId,
  enabled = true,
}: {
  assetId: string;
  enabled?: boolean;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(assetId),
    queryKey: attachmentQueryKeys.asset(apiBaseUrl, assetId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return attachmentListSchema.parse(await client.get(`/assets/${assetId}/attachments`));
    },
  });
}

export function useInventoryItemAttachments({
  enabled = true,
  inventoryItemId,
}: {
  enabled?: boolean;
  inventoryItemId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(inventoryItemId),
    queryKey: attachmentQueryKeys.inventoryItem(apiBaseUrl, inventoryItemId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return attachmentListSchema.parse(
        await client.get(`/inventory-items/${inventoryItemId}/attachments`),
      );
    },
  });
}

export function useUploadFile() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({ file, laboratoryId }: { file: File; laboratoryId: string }) => {
      const client = createApiClient(apiBaseUrl);
      const form = new FormData();
      form.append("file", file);
      return fileUploadSchema.parse(
        await client.postFormData(`/laboratories/${laboratoryId}/file-uploads`, form),
      );
    },
  });
}

export function useDeleteFileUpload() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (uploadId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/file-uploads/${uploadId}`);
    },
  });
}

export function useCreateAssetAttachment() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ assetId, claim }: { assetId: string; claim: AttachmentClaim }) => {
      const client = createApiClient(apiBaseUrl);
      return attachmentSchema.parse(await client.post(`/assets/${assetId}/attachments`, claim));
    },
    onSuccess: (_attachment, variables) => {
      queryClient.invalidateQueries({
        queryKey: attachmentQueryKeys.asset(apiBaseUrl, variables.assetId),
      });
    },
  });
}

export function useCreateInventoryItemAttachment() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      claim,
      inventoryItemId,
    }: {
      claim: AttachmentClaim;
      inventoryItemId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return attachmentSchema.parse(
        await client.post(`/inventory-items/${inventoryItemId}/attachments`, claim),
      );
    },
    onSuccess: (_attachment, variables) => {
      queryClient.invalidateQueries({
        queryKey: attachmentQueryKeys.inventoryItem(apiBaseUrl, variables.inventoryItemId),
      });
    },
  });
}

export function useDeleteAttachment() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (attachmentId: string) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/attachments/${attachmentId}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: attachmentQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useDownloadAttachment() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (attachmentId: string) => {
      const client = createApiClient(apiBaseUrl);
      return client.downloadBlob(`/attachments/${attachmentId}/download`);
    },
  });
}

export async function deleteFileUploads(apiBaseUrl: string, uploadIds: string[]) {
  if (uploadIds.length === 0) {
    return;
  }
  const client = createApiClient(apiBaseUrl);
  await Promise.allSettled(
    uploadIds.map((uploadId) => client.delete(`/file-uploads/${uploadId}`)),
  );
}
