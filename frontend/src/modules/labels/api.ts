import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
import { isSystemAdmin } from "../auth/permissions";
import { localLaboratoryPath } from "../federation/scope";

/**
 * Everything the client needs to lay a label out at the size the printer will
 * accept. The server owns these numbers because it owns the raster layer.
 */
export const labelLayoutSchema = z.object({
  dpi: z.number(),
  max_length_dots: z.number(),
  min_length_dots: z.number(),
  /** Zero for continuous stock, where the caller chooses the length. */
  printable_length_dots: z.number(),
  printable_width_dots: z.number(),
});

export const labelPrinterSchema = z.object({
  auto_cut: z.boolean(),
  created_at: z.string(),
  host: z.string(),
  laboratory_id: z.string().uuid(),
  /** Null when the configured stock is not a size the server supports. */
  layout: labelLayoutSchema.nullable(),
  media_kind: z.string(),
  media_length_mm: z.number().nullable(),
  media_width_mm: z.number(),
  model: z.string(),
  name: z.string(),
  port: z.number(),
  printer_id: z.string().uuid(),
  updated_at: z.string(),
});

const labelPrintersSchema = z.array(labelPrinterSchema);

export const printerFaultSchema = z.enum([
  "no_media",
  "end_of_media",
  "cutter_jam",
  "main_unit_in_use",
  "printer_turned_off",
  "high_voltage_adapter",
  "fan_error",
  "replace_media",
  "expansion_buffer_full",
  "communication_error",
  "communication_buffer_full",
  "cover_open",
  "cancel_key",
  "cannot_feed_media",
  "system_error",
]);

export const printerStatusSchema = z.object({
  faults: z.array(printerFaultSchema),
  media_kind: z.string().nullable(),
  media_length_mm: z.number(),
  media_matches_configuration: z.boolean(),
  media_width_mm: z.number(),
  ready: z.boolean(),
});

export const instanceIdentitySchema = z.object({
  node_id: z.string().uuid(),
  public_web_url: z.string(),
});

const printLabelsResultSchema = z.object({
  labels_printed: z.number(),
});

export type LabelLayout = z.infer<typeof labelLayoutSchema>;
export type LabelPrinter = z.infer<typeof labelPrinterSchema>;
export type PrinterFault = z.infer<typeof printerFaultSchema>;
export type PrinterStatus = z.infer<typeof printerStatusSchema>;
export type InstanceIdentity = z.infer<typeof instanceIdentitySchema>;

export type LabelPrinterPayload = {
  auto_cut?: boolean;
  host: string;
  media_kind: string;
  media_length_mm?: number | null;
  media_width_mm: number;
  model?: string;
  name: string;
  port?: number;
};

export type LabelPage = {
  bitmap_base64: string;
  height_dots: number;
  width_dots: number;
};

/** Human wording for what the printer is complaining about. */
export const printerFaultLabels: Record<PrinterFault, string> = {
  cancel_key: "取消键被按下",
  cannot_feed_media: "无法走纸",
  communication_buffer_full: "通信缓冲区已满",
  communication_error: "通信错误",
  cover_open: "机盖未合上",
  cutter_jam: "切刀卡住",
  end_of_media: "标签纸已用完",
  expansion_buffer_full: "缓冲区已满",
  fan_error: "风扇故障",
  high_voltage_adapter: "电源适配器异常",
  main_unit_in_use: "打印机正忙",
  no_media: "未装入标签纸",
  printer_turned_off: "打印机已关闭",
  replace_media: "请更换标签纸",
  system_error: "系统错误",
};

export function describeMedia(printer: LabelPrinter) {
  return printer.media_kind === "die_cut"
    ? `${printer.media_width_mm}×${printer.media_length_mm}mm 模切标签`
    : `${printer.media_width_mm}mm 连续纸`;
}

export const labelQueryKeys = {
  instanceIdentity: (apiBaseUrl: string) =>
    ["labels", apiBaseUrl, "instance-identity"] as const,
  printerStatus: (apiBaseUrl: string, printerId: string) =>
    ["labels", apiBaseUrl, "printer-status", printerId] as const,
  printers: (apiBaseUrl: string, laboratoryId: string) =>
    ["labels", apiBaseUrl, "printers", laboratoryId] as const,
  root: (apiBaseUrl: string) => ["labels", apiBaseUrl] as const,
};

/**
 * This deployment's federation node id and web origin.
 *
 * Every QR code embeds these, and neither changes while the server is running,
 * so it is fetched once and held.
 */
export function useInstanceIdentity({ enabled = true } = {}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled,
    queryKey: labelQueryKeys.instanceIdentity(apiBaseUrl),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return instanceIdentitySchema.parse(await client.get("/instance-identity"));
    },
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useLabelPrinters({
  enabled = true,
  laboratoryId,
}: {
  enabled?: boolean;
  laboratoryId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: labelQueryKeys.printers(apiBaseUrl, laboratoryId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return labelPrintersSchema.parse(
        await client.get(
          localLaboratoryPath(laboratoryId, "label-printers", isSystemAdmin(currentUser)),
        ),
      );
    },
  });
}

/**
 * Asks the printer what it is actually loaded with.
 *
 * Not cached: the answer is about the physical state of a device, and a stale
 * "ready" is worse than no answer at all.
 */
export function useLabelPrinterStatus({
  enabled = true,
  laboratoryId,
  printerId,
}: {
  enabled?: boolean;
  laboratoryId: string;
  printerId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useQuery({
    enabled: enabled && Boolean(printerId),
    queryKey: labelQueryKeys.printerStatus(apiBaseUrl, printerId),
    gcTime: 0,
    staleTime: 0,
    retry: false,
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return printerStatusSchema.parse(
        await client.get(
          localLaboratoryPath(
            laboratoryId,
            `label-printers/${printerId}/status`,
            isSystemAdmin(currentUser),
          ),
        ),
      );
    },
  });
}

export function useCreateLabelPrinter(laboratoryId: string) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (payload: LabelPrinterPayload) => {
      const client = createApiClient(apiBaseUrl);
      return labelPrinterSchema.parse(
        await client.post(
          localLaboratoryPath(laboratoryId, "label-printers", isSystemAdmin(currentUser)),
          payload,
        ),
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useUpdateLabelPrinter(laboratoryId: string) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      payload,
      printerId,
    }: {
      payload: Partial<LabelPrinterPayload>;
      printerId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return labelPrinterSchema.parse(
        await client.patch(
          localLaboratoryPath(
            laboratoryId,
            `label-printers/${printerId}`,
            isSystemAdmin(currentUser),
          ),
          payload,
        ),
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function useDeleteLabelPrinter(laboratoryId: string) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (printerId: string) => {
      const client = createApiClient(apiBaseUrl);
      return client.delete(
        localLaboratoryPath(
          laboratoryId,
          `label-printers/${printerId}`,
          isSystemAdmin(currentUser),
        ),
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelQueryKeys.root(apiBaseUrl) });
    },
  });
}

export function usePrintLabels(laboratoryId: string) {
  const { apiBaseUrl } = useBackendConfig();
  const { currentUser } = useAuth();

  return useMutation({
    mutationFn: async ({
      copies,
      pages,
      printerId,
    }: {
      copies: number;
      pages: LabelPage[];
      printerId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return printLabelsResultSchema.parse(
        await client.post(
          localLaboratoryPath(
            laboratoryId,
            `label-printers/${printerId}/print`,
            isSystemAdmin(currentUser),
          ),
          { copies, pages },
        ),
      );
    },
  });
}
