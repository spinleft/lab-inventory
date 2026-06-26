import { describe, expect, it } from "vitest";
import {
  type PendingAttachment,
  attachmentClaimsFromPending,
  pendingAttachmentFromUpload,
} from "./AttachmentPanel";

describe("attachment pending claims", () => {
  it("requires a visibility choice for every pending attachment", () => {
    const result = attachmentClaimsFromPending([pendingAttachment()]);

    expect(result.ok).toBe(false);
  });

  it("serializes pending attachments into backend claims", () => {
    const result = attachmentClaimsFromPending([
      {
        ...pendingAttachment(),
        description: "  calibration certificate  ",
        displayName: "  Certificate.pdf  ",
        visibility: "public",
      },
    ]);

    expect(result).toEqual({
      claims: [
        {
          description: "calibration certificate",
          display_name: "Certificate.pdf",
          upload_id: "00000000-0000-4000-8000-000000000101",
          visibility: "public",
        },
      ],
      ok: true,
    });
  });

  it("uses the uploaded file name when display name is blank", () => {
    const pending = pendingAttachmentFromUpload({
      created_at: "2026-06-26T00:00:00Z",
      expires_at: "2026-06-26T01:00:00Z",
      file_size_bytes: 512,
      laboratory_id: "00000000-0000-4000-8000-000000000201",
      mime_type: "application/pdf",
      original_file_name: "manual.pdf",
      sha256_hex: "a".repeat(64),
      upload_id: "00000000-0000-4000-8000-000000000101",
    });

    const result = attachmentClaimsFromPending([
      { ...pending, displayName: "", visibility: "internal" },
    ]);

    expect(result).toEqual({
      claims: [
        {
          description: null,
          display_name: "manual.pdf",
          upload_id: "00000000-0000-4000-8000-000000000101",
          visibility: "internal",
        },
      ],
      ok: true,
    });
  });
});

function pendingAttachment(): PendingAttachment {
  return {
    description: "",
    displayName: "manual.pdf",
    fileSizeBytes: 512,
    mimeType: "application/pdf",
    originalFileName: "manual.pdf",
    uploadId: "00000000-0000-4000-8000-000000000101",
    visibility: "",
  };
}
