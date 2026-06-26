import { Download, Paperclip, Trash2, UploadCloud, X } from "lucide-react";
import {
  type ChangeEvent,
  type Dispatch,
  type SetStateAction,
  useEffect,
  useRef,
  useState,
} from "react";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { toErrorMessage } from "../../shared/lib/errors";
import { formatDate } from "../../shared/lib/date";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { ConfirmDialog } from "../../shared/ui/ConfirmDialog";
import { EmptyState } from "../../shared/ui/EmptyState";
import { FormField } from "../../shared/ui/FormField";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import {
  type Attachment,
  type AttachmentClaim,
  type AttachmentUpload,
  type AttachmentVisibility,
  deleteAttachmentUploads,
  useAssetAttachments,
  useCreateAssetAttachment,
  useCreateInventoryItemAttachment,
  useDeleteAttachment,
  useDeleteAttachmentUpload,
  useDownloadAttachment,
  useInventoryItemAttachments,
  useUploadAttachment,
} from "./api";

export type PendingAttachment = {
  description: string;
  displayName: string;
  fileSizeBytes: number;
  mimeType: string | null;
  originalFileName: string;
  uploadId: string;
  visibility: AttachmentVisibility | "";
};

type AttachmentTarget =
  | { id: string; type: "asset" }
  | { id: string; type: "inventory-item" };

type ClaimsResult =
  | { claims: AttachmentClaim[]; ok: true }
  | { message: string; ok: false };

const VISIBILITY_OPTIONS = [
  { label: "选择可见性", value: "none" },
  { label: "公开", value: "public" },
  { label: "内部", value: "internal" },
];

export function attachmentClaimsFromPending(pendingAttachments: PendingAttachment[]): ClaimsResult {
  const claims: AttachmentClaim[] = [];
  for (const attachment of pendingAttachments) {
    if (!attachment.visibility) {
      return { message: "请选择每个附件的可见性。", ok: false };
    }
    const displayName = attachment.displayName.trim() || attachment.originalFileName;
    const description = attachment.description.trim();
    claims.push({
      description: description || null,
      display_name: displayName,
      upload_id: attachment.uploadId,
      visibility: attachment.visibility,
    });
  }
  return { claims, ok: true };
}

export function pendingAttachmentFromUpload(upload: AttachmentUpload): PendingAttachment {
  return {
    description: "",
    displayName: upload.original_file_name,
    fileSizeBytes: upload.file_size_bytes,
    mimeType: upload.mime_type,
    originalFileName: upload.original_file_name,
    uploadId: upload.upload_id,
    visibility: "",
  };
}

export function pendingAttachmentUploadIds(pendingAttachments: PendingAttachment[]) {
  return pendingAttachments.map((attachment) => attachment.uploadId);
}

export function PendingAttachmentUploader({
  cleanupOnUnmount = true,
  disabled = false,
  laboratoryId,
  onChange,
  pendingAttachments,
}: {
  cleanupOnUnmount?: boolean;
  disabled?: boolean;
  laboratoryId: string;
  onChange: Dispatch<SetStateAction<PendingAttachment[]>>;
  pendingAttachments: PendingAttachment[];
}) {
  const { apiBaseUrl } = useBackendConfig();
  const toast = useToast();
  const uploadAttachment = useUploadAttachment();
  const deleteUpload = useDeleteAttachmentUpload();
  const pendingRef = useRef(pendingAttachments);
  const [uploadingCount, setUploadingCount] = useState(0);

  useEffect(() => {
    pendingRef.current = pendingAttachments;
  }, [pendingAttachments]);

  useEffect(() => {
    return () => {
      if (!cleanupOnUnmount) {
        return;
      }
      void deleteAttachmentUploads(apiBaseUrl, pendingAttachmentUploadIds(pendingRef.current));
    };
  }, [apiBaseUrl, cleanupOnUnmount]);

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0 || disabled) {
      return;
    }
    if (!laboratoryId) {
      toast.error({ title: "请先选择实验室。" });
      return;
    }

    setUploadingCount(files.length);
    try {
      for (const file of files) {
        const upload = await uploadAttachment.mutateAsync({ file, laboratoryId });
        onChange((current) => [...current, pendingAttachmentFromUpload(upload)]);
      }
      toast.success({ title: "附件已上传，请选择可见性后保存。" });
    } catch (error) {
      toast.error({ title: "附件上传失败", description: toErrorMessage(error) });
    } finally {
      setUploadingCount(0);
    }
  }

  async function removePendingAttachment(uploadId: string) {
    try {
      await deleteUpload.mutateAsync(uploadId);
      onChange((current) => current.filter((attachment) => attachment.uploadId !== uploadId));
    } catch (error) {
      toast.error({ title: "删除临时附件失败", description: toErrorMessage(error) });
    }
  }

  function updatePendingAttachment(uploadId: string, patch: Partial<PendingAttachment>) {
    onChange((current) =>
      current.map((attachment) =>
        attachment.uploadId === uploadId ? { ...attachment, ...patch } : attachment,
      ),
    );
  }

  const uploading = uploadingCount > 0 || uploadAttachment.isPending;

  return (
    <div className="attachment-uploader">
      <div className="attachment-uploader-header">
        <div>
          <h3 className="asset-editor-section-title">附件</h3>
          <p className="panel-description">每个附件必须选择公开或内部后才能保存。</p>
        </div>
        <label className="button attachment-upload-button">
          <UploadCloud size={15} />
          {uploading ? "上传中" : "选择文件"}
          <input
            disabled={disabled || uploading}
            multiple
            type="file"
            onChange={handleFileChange}
          />
        </label>
      </div>

      {pendingAttachments.length === 0 ? (
        <p className="attachment-empty">还没有待保存附件。</p>
      ) : (
        <div className="attachment-pending-list">
          {pendingAttachments.map((attachment) => (
            <div className="attachment-pending-item" key={attachment.uploadId}>
              <div className="attachment-pending-meta">
                <Paperclip size={16} />
                <div>
                  <strong>{attachment.originalFileName}</strong>
                  <span>{formatFileSize(attachment.fileSizeBytes)}</span>
                </div>
              </div>
              <div className="form-grid form-grid-2">
                <FormField htmlFor={`attachment-name-${attachment.uploadId}`} label="显示名">
                  <input
                    className="input"
                    id={`attachment-name-${attachment.uploadId}`}
                    value={attachment.displayName}
                    onChange={(event) =>
                      updatePendingAttachment(attachment.uploadId, {
                        displayName: event.target.value,
                      })
                    }
                  />
                </FormField>
                <FormField htmlFor={`attachment-visibility-${attachment.uploadId}`} label="可见性">
                  <Select
                    id={`attachment-visibility-${attachment.uploadId}`}
                    label="附件可见性"
                    options={VISIBILITY_OPTIONS}
                    value={attachment.visibility || "none"}
                    onValueChange={(value) =>
                      updatePendingAttachment(attachment.uploadId, {
                        visibility:
                          value === "none" ? "" : (value as AttachmentVisibility),
                      })
                    }
                  />
                </FormField>
              </div>
              <FormField htmlFor={`attachment-description-${attachment.uploadId}`} label="描述">
                <textarea
                  className="textarea"
                  id={`attachment-description-${attachment.uploadId}`}
                  value={attachment.description}
                  onChange={(event) =>
                    updatePendingAttachment(attachment.uploadId, {
                      description: event.target.value,
                    })
                  }
                />
              </FormField>
              <div className="attachment-pending-actions">
                <Button
                  disabled={deleteUpload.isPending}
                  variant="ghost"
                  onClick={() => void removePendingAttachment(attachment.uploadId)}
                >
                  <X size={15} />
                  移除附件
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function AttachmentSection({
  canManage,
  laboratoryId,
  target,
}: {
  canManage: boolean;
  laboratoryId: string;
  target: AttachmentTarget;
}) {
  const toast = useToast();
  const assetAttachments = useAssetAttachments({
    assetId: target.type === "asset" ? target.id : "",
    enabled: target.type === "asset",
  });
  const inventoryAttachments = useInventoryItemAttachments({
    enabled: target.type === "inventory-item",
    inventoryItemId: target.type === "inventory-item" ? target.id : "",
  });
  const createAssetAttachment = useCreateAssetAttachment();
  const createInventoryItemAttachment = useCreateInventoryItemAttachment();
  const deleteAttachment = useDeleteAttachment();
  const downloadAttachment = useDownloadAttachment();
  const [pendingAttachments, setPendingAttachments] = useState<PendingAttachment[]>([]);

  const query = target.type === "asset" ? assetAttachments : inventoryAttachments;
  const attachments = query.data ?? [];
  const isBinding = createAssetAttachment.isPending || createInventoryItemAttachment.isPending;

  async function bindPendingAttachments() {
    const result = attachmentClaimsFromPending(pendingAttachments);
    if (!result.ok) {
      toast.error({ title: result.message });
      return;
    }
    try {
      for (const claim of result.claims) {
        if (target.type === "asset") {
          await createAssetAttachment.mutateAsync({ assetId: target.id, claim });
        } else {
          await createInventoryItemAttachment.mutateAsync({
            claim,
            inventoryItemId: target.id,
          });
        }
      }
      setPendingAttachments([]);
      toast.success({ title: "附件已添加。" });
    } catch (error) {
      toast.error({ title: "添加附件失败", description: toErrorMessage(error) });
    }
  }

  async function deleteBoundAttachment(attachmentId: string) {
    try {
      await deleteAttachment.mutateAsync(attachmentId);
      toast.success({ title: "附件已删除。" });
    } catch (error) {
      toast.error({ title: "删除附件失败", description: toErrorMessage(error) });
    }
  }

  async function downloadBoundAttachment(attachment: Attachment) {
    try {
      const download = await downloadAttachment.mutateAsync(attachment.attachment_id);
      saveDownload(download.blob, download.fileName ?? attachment.original_file_name);
    } catch (error) {
      toast.error({ title: "下载附件失败", description: toErrorMessage(error) });
    }
  }

  return (
    <section className="panel">
      <div className="panel-header">
        <div>
          <h2 className="panel-title">附件</h2>
          <p className="panel-description">{attachments.length} 个附件</p>
        </div>
      </div>
      <div className="panel-body attachment-section-body">
        {attachments.length === 0 ? (
          <EmptyState description="当前记录还没有附件。" title="暂无附件" />
        ) : (
          <div className="attachment-list">
            {attachments.map((attachment) => (
              <div className="attachment-list-item" key={attachment.attachment_id}>
                <div className="attachment-list-main">
                  <Paperclip size={18} />
                  <div>
                    <strong>{attachment.display_name}</strong>
                    <span>
                      {attachment.original_file_name} · {formatFileSize(attachment.file_size_bytes)} ·{" "}
                      {formatDate(attachment.created_at)}
                    </span>
                    {attachment.description ? <p>{attachment.description}</p> : null}
                  </div>
                </div>
                <div className="attachment-list-actions">
                  <Badge tone={attachment.visibility === "public" ? "success" : "warning"}>
                    {attachment.visibility === "public" ? "公开" : "内部"}
                  </Badge>
                  <Button
                    disabled={downloadAttachment.isPending}
                    size="icon"
                    variant="ghost"
                    aria-label={`下载附件 ${attachment.display_name}`}
                    onClick={() => void downloadBoundAttachment(attachment)}
                  >
                    <Download size={15} />
                  </Button>
                  {canManage ? (
                    <ConfirmDialog
                      confirmLabel="删除"
                      description="删除后该附件将无法继续访问。"
                      title="删除附件"
                      trigger={
                        <Button
                          disabled={deleteAttachment.isPending}
                          size="icon"
                          variant="ghost"
                          aria-label={`删除附件 ${attachment.display_name}`}
                        >
                          <Trash2 size={15} />
                        </Button>
                      }
                      onConfirm={() => void deleteBoundAttachment(attachment.attachment_id)}
                    />
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        )}

        {canManage ? (
          <div className="attachment-detail-uploader">
            <PendingAttachmentUploader
              laboratoryId={laboratoryId}
              pendingAttachments={pendingAttachments}
              onChange={setPendingAttachments}
            />
            <div className="attachment-bind-actions">
              <Button
                disabled={pendingAttachments.length === 0 || isBinding}
                variant="primary"
                onClick={() => void bindPendingAttachments()}
              >
                <Paperclip size={15} />
                添加附件
              </Button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function saveDownload(blob: Blob, fileName: string) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
