import { Pencil, Plus, Printer, RefreshCw, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { ConfirmDialog } from "../../shared/ui/ConfirmDialog";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { Dialog } from "../../shared/ui/Dialog";
import { EmptyState } from "../../shared/ui/EmptyState";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import { canManageLabelPrinters } from "../auth/permissions";
import {
  type LabelPrinter,
  type LabelPrinterPayload,
  describeMedia,
  printerFaultLabels,
  useCreateLabelPrinter,
  useDeleteLabelPrinter,
  useLabelPrinterStatus,
  useLabelPrinters,
  useUpdateLabelPrinter,
} from "./api";

/**
 * The label stock the server knows how to lay out.
 *
 * Kept in step with `label_printing::media::SUPPORTED_MEDIA`; anything not on
 * this list is refused by the API, so offering it here would only produce a
 * confusing error.
 */
const MEDIA_OPTIONS = [
  { label: "62mm 连续纸", value: "continuous:62:" },
  { label: "50mm 连续纸", value: "continuous:50:" },
  { label: "38mm 连续纸", value: "continuous:38:" },
  { label: "29mm 连续纸", value: "continuous:29:" },
  { label: "62×29mm 模切", value: "die_cut:62:29" },
  { label: "62×100mm 模切", value: "die_cut:62:100" },
  { label: "29×90mm 模切", value: "die_cut:29:90" },
  { label: "23×23mm 模切", value: "die_cut:23:23" },
  { label: "17×54mm 模切", value: "die_cut:17:54" },
];

const DEFAULT_MEDIA = "die_cut:62:29";

type PrinterForm = {
  auto_cut: boolean;
  host: string;
  media: string;
  model: string;
  name: string;
  port: string;
};

function mediaValue(printer: LabelPrinter) {
  return `${printer.media_kind}:${printer.media_width_mm}:${printer.media_length_mm ?? ""}`;
}

function formFor(printer: LabelPrinter | "new"): PrinterForm {
  if (printer === "new") {
    return {
      auto_cut: true,
      host: "",
      media: DEFAULT_MEDIA,
      model: "QL-820NWBc",
      name: "",
      port: "9100",
    };
  }
  return {
    auto_cut: printer.auto_cut,
    host: printer.host,
    media: mediaValue(printer),
    model: printer.model,
    name: printer.name,
    port: String(printer.port),
  };
}

function payloadFrom(form: PrinterForm): LabelPrinterPayload {
  const [kind, width, length] = form.media.split(":");
  return {
    auto_cut: form.auto_cut,
    host: form.host.trim(),
    media_kind: kind,
    media_length_mm: length ? Number(length) : null,
    media_width_mm: Number(width),
    model: form.model.trim(),
    name: form.name.trim(),
    port: Number(form.port),
  };
}

export function LabelPrintersPage() {
  const { currentUser } = useAuth();
  const { selectedLaboratoryId } = useLaboratorySelection();
  const toast = useToast();
  const printersQuery = useLabelPrinters({ laboratoryId: selectedLaboratoryId });
  const createPrinter = useCreateLabelPrinter(selectedLaboratoryId);
  const updatePrinter = useUpdateLabelPrinter(selectedLaboratoryId);
  const deletePrinter = useDeleteLabelPrinter(selectedLaboratoryId);
  const canManage = canManageLabelPrinters(currentUser);

  const [editing, setEditing] = useState<LabelPrinter | "new" | null>(null);
  const [testing, setTesting] = useState<LabelPrinter | null>(null);

  const printers = printersQuery.data ?? [];

  async function handleDelete(printer: LabelPrinter) {
    try {
      await deletePrinter.mutateAsync(printer.printer_id);
      toast.success({ title: "打印机已删除" });
    } catch (error) {
      toast.error({ title: "删除失败", description: toErrorMessage(error) });
    }
  }

  const columns: DataTableColumn<LabelPrinter>[] = [
    { header: "名称", key: "name", render: (printer) => printer.name },
    {
      header: "地址",
      key: "address",
      render: (printer) => (
        <span className="mono">
          {printer.host}:{printer.port}
        </span>
      ),
    },
    { header: "型号", key: "model", render: (printer) => printer.model },
    {
      header: "标签纸",
      key: "media",
      render: (printer) =>
        printer.layout ? (
          describeMedia(printer)
        ) : (
          <Badge tone="danger">规格不受支持</Badge>
        ),
    },
    {
      header: "自动切纸",
      key: "auto_cut",
      render: (printer) => (printer.auto_cut ? "是" : "否"),
    },
    {
      header: "更新时间",
      key: "updated_at",
      render: (printer) => formatDate(printer.updated_at),
    },
    {
      align: "right",
      header: "操作",
      key: "actions",
      render: (printer) => (
        <span className="table-actions">
          <Button
            aria-label="测试连接"
            size="icon"
            variant="ghost"
            onClick={() => setTesting(printer)}
          >
            <RefreshCw size={15} />
          </Button>
          <Button
            aria-label="编辑"
            disabled={!canManage}
            size="icon"
            variant="ghost"
            onClick={() => setEditing(printer)}
          >
            <Pencil size={15} />
          </Button>
          <ConfirmDialog
            confirmLabel="删除"
            description={`确认删除打印机“${printer.name}”？此操作不影响已打印的标签。`}
            disabled={!canManage || deletePrinter.isPending}
            title="删除打印机"
            trigger={
              <Button aria-label="删除" disabled={!canManage} size="icon" variant="ghost">
                <Trash2 size={15} />
              </Button>
            }
            onConfirm={() => handleDelete(printer)}
          />
        </span>
      ),
    },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="系统管理"
        title="标签打印机"
        actions={
          canManage ? (
            <Button disabled={!selectedLaboratoryId} onClick={() => setEditing("new")}>
              <Plus size={16} />
              添加打印机
            </Button>
          ) : null
        }
      />

      <section className="panel">
        {printers.length === 0 && !printersQuery.isLoading ? (
          <EmptyState
            title="暂无标签打印机"
            description="添加一台网络标签打印机后，就可以从资产和库存页面打印二维码标签。"
          />
        ) : (
          <DataTable
            columns={columns}
            getRowKey={(printer) => printer.printer_id}
            items={printers}
            loading={printersQuery.isLoading}
          />
        )}
      </section>

      {editing ? (
        <PrinterEditor
          key={editing === "new" ? "new" : editing.printer_id}
          printer={editing}
          onClose={() => setEditing(null)}
          onSubmit={async (form) => {
            const payload = payloadFrom(form);
            try {
              if (editing === "new") {
                await createPrinter.mutateAsync(payload);
                toast.success({ title: "打印机已添加" });
              } else {
                await updatePrinter.mutateAsync({
                  payload,
                  printerId: editing.printer_id,
                });
                toast.success({ title: "打印机已更新" });
              }
              setEditing(null);
            } catch (error) {
              toast.error({ title: "保存失败", description: toErrorMessage(error) });
            }
          }}
          pending={createPrinter.isPending || updatePrinter.isPending}
        />
      ) : null}

      {testing ? (
        <PrinterStatusDialog
          laboratoryId={selectedLaboratoryId}
          printer={testing}
          onClose={() => setTesting(null)}
        />
      ) : null}

    </main>
  );
}

type PrinterEditorProps = {
  onClose: () => void;
  onSubmit: (form: PrinterForm) => void;
  pending: boolean;
  printer: LabelPrinter | "new";
};

function PrinterEditor({ onClose, onSubmit, pending, printer }: PrinterEditorProps) {
  const [form, setForm] = useState<PrinterForm>(() => formFor(printer));

  function update<Key extends keyof PrinterForm>(key: Key, value: PrinterForm[Key]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    onSubmit(form);
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={printer === "new" ? "添加标签打印机" : "编辑标签打印机"}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button
            disabled={pending || !form.name.trim() || !form.host.trim()}
            form="label-printer-form"
            type="submit"
          >
            {pending ? "保存中…" : "保存"}
          </Button>
        </>
      }
    >
      <form className="form-grid" id="label-printer-form" onSubmit={handleSubmit}>
        <FormField htmlFor="printer-name" label="名称">
          <input
            className="input"
            id="printer-name"
            placeholder="例如：前台标签机"
            value={form.name}
            onChange={(event) => update("name", event.target.value)}
          />
        </FormField>

        <FormField
          htmlFor="printer-host"
          label="IP 地址或主机名"
          hint="打印机需与服务器网络互通。不要带端口或 http:// 前缀。"
        >
          <input
            className="input"
            id="printer-host"
            placeholder="192.168.1.50"
            value={form.host}
            onChange={(event) => update("host", event.target.value)}
          />
        </FormField>

        <FormField htmlFor="printer-port" label="端口" hint="Brother 网络打印机通常为 9100。">
          <input
            className="input"
            id="printer-port"
            inputMode="numeric"
            value={form.port}
            onChange={(event) => update("port", event.target.value)}
          />
        </FormField>

        <FormField htmlFor="printer-model" label="型号">
          <input
            className="input"
            id="printer-model"
            value={form.model}
            onChange={(event) => update("model", event.target.value)}
          />
        </FormField>

        <FormField
          htmlFor="printer-media"
          label="标签纸规格"
          hint="需与打印机实际装入的纸卷一致，否则打印会被拒绝。"
        >
          <Select
            id="printer-media"
            onValueChange={(value) => update("media", value)}
            options={MEDIA_OPTIONS}
            value={form.media}
          />
        </FormField>

        <label className="checkbox-field">
          <input
            checked={form.auto_cut}
            onChange={(event) => update("auto_cut", event.target.checked)}
            type="checkbox"
          />
          <span>打印后自动切纸</span>
        </label>
      </form>
    </Dialog>
  );
}

type PrinterStatusDialogProps = {
  laboratoryId: string;
  onClose: () => void;
  printer: LabelPrinter;
};

/** Reads the printer's own report of what it is loaded with. */
function PrinterStatusDialog({
  laboratoryId,
  onClose,
  printer,
}: PrinterStatusDialogProps) {
  const statusQuery = useLabelPrinterStatus({
    laboratoryId,
    printerId: printer.printer_id,
  });

  // Always ask again when the dialog opens: the point is the current state of a
  // physical device.
  useEffect(() => {
    void statusQuery.refetch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [printer.printer_id]);

  const status = statusQuery.data;

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      title={`打印机状态 — ${printer.name}`}
      footer={
        <Button variant="ghost" onClick={onClose}>
          关闭
        </Button>
      }
    >
      {statusQuery.isFetching ? (
        <div className="skeleton" style={{ height: 90 }} />
      ) : statusQuery.isError ? (
        <p className="field-error">
          无法连接打印机：{toErrorMessage(statusQuery.error)}
        </p>
      ) : status ? (
        <dl className="asset-detail-grid">
          <div>
            <dt>连接</dt>
            <dd>
              <Badge tone="success">已连接</Badge>
            </dd>
          </div>
          <div>
            <dt>实际装入</dt>
            <dd>
              {status.media_kind === null
                ? "未装入标签纸"
                : status.media_kind === "die_cut"
                  ? `${status.media_width_mm}×${status.media_length_mm}mm 模切标签`
                  : `${status.media_width_mm}mm 连续纸`}
            </dd>
          </div>
          <div>
            <dt>与配置一致</dt>
            <dd>
              {status.media_matches_configuration ? (
                <Badge tone="success">一致</Badge>
              ) : (
                <Badge tone="danger">不一致</Badge>
              )}
            </dd>
          </div>
          <div>
            <dt>就绪状态</dt>
            <dd>
              {status.ready ? (
                <Badge tone="success">就绪</Badge>
              ) : (
                status.faults.map((fault) => printerFaultLabels[fault]).join("、")
              )}
            </dd>
          </div>
        </dl>
      ) : null}
    </Dialog>
  );
}

export { Printer as LabelPrinterIcon };
