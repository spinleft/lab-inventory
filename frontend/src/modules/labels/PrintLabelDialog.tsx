import { AlertTriangle, Printer } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { toErrorMessage } from "../../shared/lib/errors";
import { buildScanPayload } from "../../shared/lib/qrPayload";
import { Button } from "../../shared/ui/Button";
import { Dialog } from "../../shared/ui/Dialog";
import { FormField } from "../../shared/ui/FormField";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import { canManageLabelPrinters } from "../auth/permissions";
import {
  type LabelPage,
  describeMedia,
  printerFaultLabels,
  useInstanceIdentity,
  useLabelPrinterStatus,
  useLabelPrinters,
  usePrintLabels,
} from "./api";
import { packCanvas } from "./bitmap";
import { type LabelContent, renderLabel } from "./renderLabel";

/** One thing to put on a label, before the QR payload is built for it. */
export type LabelSubject = {
  code?: string | null;
  resourceId: string;
  subtitle?: string | null;
  title: string;
  type: "asset" | "item";
};

type PrintLabelDialogProps = {
  laboratoryId: string;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  subjects: LabelSubject[];
};

const MAX_COPIES = 20;

export function PrintLabelDialog({
  laboratoryId,
  onOpenChange,
  open,
  subjects,
}: PrintLabelDialogProps) {
  const { currentUser } = useAuth();
  const toast = useToast();
  const [printerId, setPrinterId] = useState("");
  const [copies, setCopies] = useState(1);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const previewRef = useRef<HTMLDivElement | null>(null);

  const printersQuery = useLabelPrinters({ enabled: open, laboratoryId });
  const identityQuery = useInstanceIdentity({ enabled: open });
  const printLabels = usePrintLabels(laboratoryId);

  const printers = useMemo(() => printersQuery.data ?? [], [printersQuery.data]);
  const printer = printers.find((candidate) => candidate.printer_id === printerId);

  const statusQuery = useLabelPrinterStatus({
    enabled: open && Boolean(printerId),
    laboratoryId,
    printerId,
  });

  // Settle on a printer as soon as the list arrives, so the preview has
  // dimensions to render at without the user picking first.
  useEffect(() => {
    if (!open) {
      return;
    }
    if (printers.length > 0 && !printers.some((p) => p.printer_id === printerId)) {
      setPrinterId(printers[0].printer_id);
    }
  }, [open, printerId, printers]);

  const identity = identityQuery.data;
  const layout = printer?.layout ?? null;

  /** Builds the label content for a subject, including its QR payload. */
  const contentFor = useMemo(() => {
    if (!identity) {
      return null;
    }
    return (subject: LabelSubject): LabelContent => ({
      code: subject.code,
      payload: buildScanPayload(identity.public_web_url, {
        laboratoryId,
        nodeId: identity.node_id,
        resourceId: subject.resourceId,
        type: subject.type,
      }),
      subtitle: subject.subtitle,
      title: subject.title,
    });
  }, [identity, laboratoryId]);

  // Preview the first label. Rendering happens off-DOM and the canvas is
  // inserted as-is, scaled only by CSS, so the preview is the printed bitmap.
  useEffect(() => {
    const container = previewRef.current;
    if (!open || !container || !layout || !contentFor || subjects.length === 0) {
      return;
    }

    let cancelled = false;
    setPreviewError(null);

    renderLabel(layout, contentFor(subjects[0]))
      .then((canvas) => {
        if (cancelled) {
          return;
        }
        canvas.className = "label-preview-canvas";
        container.replaceChildren(canvas);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setPreviewError(toErrorMessage(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [contentFor, layout, open, subjects]);

  const status = statusQuery.data;
  const statusError = statusQuery.isError ? toErrorMessage(statusQuery.error) : null;
  const mediaMismatch = Boolean(status && !status.media_matches_configuration);
  const faults = status?.faults ?? [];

  async function handlePrint() {
    if (!printer || !layout || !contentFor) {
      return;
    }

    try {
      const pages: LabelPage[] = [];
      for (const subject of subjects) {
        const canvas = await renderLabel(layout, contentFor(subject));
        const packed = packCanvas(canvas);
        pages.push({
          bitmap_base64: packed.bitmapBase64,
          height_dots: packed.heightDots,
          width_dots: packed.widthDots,
        });
      }

      const result = await printLabels.mutateAsync({
        copies,
        pages,
        printerId: printer.printer_id,
      });
      toast.success({
        title: "已发送打印",
        description: `共 ${result.labels_printed} 张标签。`,
      });
      onOpenChange(false);
    } catch (error) {
      toast.error({ title: "打印失败", description: toErrorMessage(error) });
    }
  }

  const totalLabels = subjects.length * copies;
  const blocked = !printer || !layout || subjects.length === 0;

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="打印标签"
      description={
        subjects.length > 1 ? `已选择 ${subjects.length} 项` : subjects[0]?.title
      }
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            disabled={blocked || printLabels.isPending}
            onClick={handlePrint}
          >
            <Printer size={16} />
            {printLabels.isPending ? "打印中…" : `打印 ${totalLabels} 张`}
          </Button>
        </>
      }
    >
      {printersQuery.isLoading ? (
        <div className="skeleton" style={{ height: 120 }} />
      ) : printers.length === 0 ? (
        <p className="field-hint">
          还没有配置标签打印机。
          {canManageLabelPrinters(currentUser)
            ? "请先在「系统管理 › 标签打印机」中添加一台。"
            : "请联系实验室管理员添加。"}
        </p>
      ) : (
        <div className="label-print-form">
          <FormField htmlFor="label-print-printer" label="打印机">
            <Select
              id="label-print-printer"
              onValueChange={setPrinterId}
              options={printers.map((candidate) => ({
                label: `${candidate.name}（${describeMedia(candidate)}）`,
                value: candidate.printer_id,
              }))}
              value={printerId}
            />
          </FormField>

          <FormField
            htmlFor="label-print-copies"
            label="每项份数"
            hint={`合计 ${totalLabels} 张`}
          >
            <input
              className="input"
              id="label-print-copies"
              max={MAX_COPIES}
              min={1}
              type="number"
              value={copies}
              onChange={(event) => {
                const next = Number(event.target.value);
                setCopies(
                  Number.isFinite(next)
                    ? Math.min(Math.max(Math.trunc(next), 1), MAX_COPIES)
                    : 1,
                );
              }}
            />
          </FormField>

          {printer && !layout ? (
            <p className="field-error">
              该打印机配置的标签规格不受支持，请检查打印机设置。
            </p>
          ) : null}

          {statusQuery.isFetching ? (
            <p className="field-hint">正在读取打印机状态…</p>
          ) : statusError ? (
            <p className="field-error">
              <AlertTriangle size={14} /> 无法连接打印机：{statusError}
            </p>
          ) : faults.length > 0 ? (
            <p className="field-error">
              <AlertTriangle size={14} /> 打印机未就绪：
              {faults.map((fault) => printerFaultLabels[fault]).join("、")}
            </p>
          ) : mediaMismatch ? (
            <p className="field-error">
              <AlertTriangle size={14} /> 打印机当前装的标签纸与配置不符，
              打印会被拒绝。请更换纸卷或修改打印机配置。
            </p>
          ) : null}

          <div className="label-preview" ref={previewRef} />
          {previewError ? <p className="field-error">{previewError}</p> : null}
        </div>
      )}
    </Dialog>
  );
}
