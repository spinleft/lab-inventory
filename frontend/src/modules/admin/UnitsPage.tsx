import { useQueryClient } from "@tanstack/react-query";
import { Pencil, Plus, Ruler, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
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
import { canManageUnits } from "../auth/permissions";
import {
  adminQueryKeys,
  type Unit,
  type UnitPayload,
  useCreateUnit,
  useDeleteUnit,
  useUnits,
  useUpdateUnit,
} from "./api";

type UnitForm = {
  allow_decimal: boolean;
  code: string;
  dimension: string;
  name: string;
  scale_to_base: string;
  symbol: string;
};

const UNIT_DIMENSION_OPTIONS = [
  { label: "数量", value: "count" },
  { label: "长度", value: "length" },
  { label: "面积", value: "area" },
  { label: "体积", value: "volume" },
  { label: "质量", value: "mass" },
  { label: "时间", value: "time" },
  { label: "温度", value: "temperature" },
  { label: "电流", value: "current" },
  { label: "光强", value: "luminous_intensity" },
  { label: "频率", value: "frequency" },
  { label: "功率", value: "power" },
  { label: "压力", value: "pressure" },
  { label: "能量", value: "energy" },
  { label: "力", value: "force" },
  { label: "扭矩", value: "torque" },
  { label: "密度", value: "density" },
];

const DEFAULT_DIMENSION = "length";

export function UnitsPage() {
  const { currentUser } = useAuth();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const unitsQuery = useUnits();
  const createUnit = useCreateUnit();
  const updateUnit = useUpdateUnit();
  const deleteUnit = useDeleteUnit();
  const canManage = canManageUnits(currentUser);
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Unit | "new" | null>(null);
  const units = unitsQuery.data ?? [];
  const filteredUnits = useMemo(
    () =>
      units.filter((unit) =>
        [
          unit.code,
          unit.name,
          unit.symbol,
          unit.dimension,
          dimensionLabel(unit.dimension),
        ]
          .join(" ")
          .toLowerCase()
          .includes(search.toLowerCase()),
      ),
    [search, units],
  );

  function refresh() {
    queryClient.invalidateQueries({ queryKey: adminQueryKeys.units(apiBaseUrl) });
  }

  function handleDelete(unit: Unit) {
    deleteUnit.mutate(unit.unit_id, {
      onError: (error) =>
        toast.error({ title: "删除单位失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "单位已删除" });
      },
    });
  }

  const columns: DataTableColumn<Unit>[] = [
    {
      header: "单位",
      key: "name",
      render: (item) => (
        <span className="unit-name-cell">
          <Ruler size={15} aria-hidden="true" />
          <strong>{item.name}</strong>
        </span>
      ),
    },
    {
      header: "代码",
      key: "code",
      render: (item) => <Badge>{item.code}</Badge>,
    },
    { header: "符号", key: "symbol", render: (item) => item.symbol },
    {
      header: "维度",
      key: "dimension",
      render: (item) => <Badge tone="accent">{dimensionLabel(item.dimension)}</Badge>,
    },
    {
      header: "基础换算系数",
      key: "scale",
      render: (item) => formatScaleToBase(item.scale_to_base),
    },
    {
      header: "小数",
      key: "decimal",
      render: (item) => (
        <Badge tone={item.allow_decimal ? "success" : "warning"}>
          {item.allow_decimal ? "允许" : "整数"}
        </Badge>
      ),
    },
    { header: "创建时间", key: "created", render: (item) => formatDate(item.created_at) },
    {
      align: "right",
      header: "操作",
      key: "actions",
      render: (item) => (
        <span className="table-actions">
          <Button
            aria-label="编辑"
            disabled={!canManage}
            size="icon"
            variant="ghost"
            onClick={() => setEditing(item)}
          >
            <Pencil size={15} />
          </Button>
          <ConfirmDialog
            confirmLabel="删除"
            description={`确认删除单位“${item.name}”？已经被资产或库存引用的单位不能删除。`}
            disabled={!canManage || deleteUnit.isPending}
            title="删除单位"
            trigger={
              <Button size="icon" variant="ghost" aria-label="删除" disabled={!canManage}>
                <Trash2 size={15} />
              </Button>
            }
            onConfirm={() => handleDelete(item)}
          />
        </span>
      ),
    },
  ];

  if (!canManage) {
    return (
      <main className="page">
        <PageHeader
          kicker="管理"
          title="单位管理"
          description="当前账号没有管理单位的权限。"
        />
        <section className="panel">
          <EmptyState description="请切换到系统管理员账号。" title="权限不足" />
        </section>
      </main>
    );
  }

  return (
    <main className="page">
      <PageHeader
        kicker="管理"
        title="单位管理"
        description="维护系统可用单位及基础单位换算系数，用于库存数量和资产参数筛选。"
        actions={
          <Button onClick={() => setEditing("new")} variant="primary">
            <Plus size={15} />
            新建单位
          </Button>
        }
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">单位列表</h2>
            <p className="panel-description">{filteredUnits.length} 个单位</p>
          </div>
          <input
            aria-label="搜索单位"
            className="input"
            placeholder="搜索单位..."
            style={{ maxWidth: 260 }}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <DataTable
          columns={columns}
          emptyDescription="当前还没有可用单位。"
          getRowKey={(item) => item.unit_id}
          items={filteredUnits}
          loading={unitsQuery.isLoading}
        />
      </section>
      <UnitEditor
        createUnit={createUnit}
        open={editing !== null}
        unit={editing}
        updateUnit={updateUnit}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          refresh();
        }}
      />
    </main>
  );
}

function UnitEditor({
  createUnit,
  onClose,
  onSaved,
  open,
  unit,
  updateUnit,
}: {
  createUnit: ReturnType<typeof useCreateUnit>;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  unit: Unit | "new" | null;
  updateUnit: ReturnType<typeof useUpdateUnit>;
}) {
  const toast = useToast();
  const isNew = unit === "new";
  const [values, setValues] = useState<UnitForm>(emptyUnitForm());
  const isSaving = createUnit.isPending || updateUnit.isPending;

  useEffect(() => {
    if (!unit || unit === "new") {
      setValues(emptyUnitForm());
      return;
    }

    setValues({
      allow_decimal: unit.allow_decimal,
      code: unit.code,
      dimension: unit.dimension,
      name: unit.name,
      scale_to_base: String(unit.scale_to_base),
      symbol: unit.symbol,
    });
  }, [unit]);

  function updateField(field: keyof UnitForm, value: string | boolean) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const scaleToBase = Number(values.scale_to_base);
    const payload: UnitPayload = {
      allow_decimal: values.allow_decimal,
      code: values.code.trim(),
      dimension: values.dimension,
      name: values.name.trim(),
      scale_to_base: scaleToBase,
      symbol: values.symbol.trim(),
    };

    if (!payload.name || !payload.code || !payload.symbol || !payload.dimension) {
      toast.error({ title: "请填写单位名称、代码、符号和维度" });
      return;
    }

    if (!Number.isFinite(scaleToBase) || scaleToBase <= 0) {
      toast.error({ title: "基础换算系数必须大于 0" });
      return;
    }

    if (isNew) {
      createUnit.mutate(payload, {
        onError: (error) =>
          toast.error({ title: "创建单位失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          toast.success({ title: "单位已创建" });
          onSaved();
        },
      });
      return;
    }

    if (unit) {
      updateUnit.mutate(
        { payload, unitId: unit.unit_id },
        {
          onError: (error) =>
            toast.error({ title: "更新单位失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "单位已更新" });
            onSaved();
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="基础换算系数用于把录入值换算到该维度的基础单位，例如 1 mm = 0.001 m。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建单位" : "编辑单位"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="unit-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="unit-form" onSubmit={handleSubmit}>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="unit-name" label="单位名称">
            <input
              className="input"
              id="unit-name"
              value={values.name}
              onChange={(event) => updateField("name", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="unit-code" label="单位代码">
            <input
              className="input"
              id="unit-code"
              value={values.code}
              onChange={(event) => updateField("code", event.target.value)}
            />
          </FormField>
        </div>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="unit-symbol" label="显示符号">
            <input
              className="input"
              id="unit-symbol"
              value={values.symbol}
              onChange={(event) => updateField("symbol", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="unit-dimension" label="单位维度">
            <Select
              id="unit-dimension"
              label="单位维度"
              options={UNIT_DIMENSION_OPTIONS}
              value={values.dimension}
              onValueChange={(value) => updateField("dimension", value)}
            />
          </FormField>
        </div>
        <FormField htmlFor="unit-scale" label="基础换算系数">
          <input
            className="input"
            id="unit-scale"
            min="0"
            step="any"
            type="number"
            value={values.scale_to_base}
            onChange={(event) => updateField("scale_to_base", event.target.value)}
          />
        </FormField>
        <label className="checkbox-field" htmlFor="unit-allow-decimal">
          <input
            checked={values.allow_decimal}
            id="unit-allow-decimal"
            type="checkbox"
            onChange={(event) => updateField("allow_decimal", event.target.checked)}
          />
          <span>
            <strong>允许小数数量</strong>
            <small>关闭后该单位按整数数量处理，例如件、台、个。</small>
          </span>
        </label>
      </form>
    </Dialog>
  );
}

function emptyUnitForm(): UnitForm {
  return {
    allow_decimal: true,
    code: "",
    dimension: DEFAULT_DIMENSION,
    name: "",
    scale_to_base: "1",
    symbol: "",
  };
}

function dimensionLabel(dimension: string) {
  return (
    UNIT_DIMENSION_OPTIONS.find((option) => option.value === dimension)?.label ?? dimension
  );
}

function formatScaleToBase(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    maximumSignificantDigits: 12,
  }).format(value);
}
