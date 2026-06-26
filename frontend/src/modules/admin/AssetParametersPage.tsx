import { useQueryClient } from "@tanstack/react-query";
import { Pencil, Plus, SlidersHorizontal, Trash2 } from "lucide-react";
import { Fragment, type FormEvent, useEffect, useMemo, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
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
import { canManageAssetParameters } from "../auth/permissions";
import {
  adminQueryKeys,
  type AssetParameter,
  type AssetParameterOptionPayload,
  type AssetParameterPayload,
  type Unit,
  optionalText,
  useAssetParameters,
  useCreateAssetParameter,
  useDeleteAssetParameter,
  useUnits,
  useUpdateAssetParameter,
} from "./api";
import {
  DEFAULT_UNIT_DIMENSION,
  UNIT_DIMENSION_OPTIONS,
  unitDimensionLabel,
} from "./unitDimensions";

type ParameterForm = {
  code: string;
  data_type: AssetParameter["data_type"];
  default_unit_id: string;
  description: string;
  is_archived: boolean;
  name: string;
  options: ParameterOptionForm[];
  unit_dimension: string;
};

type ParameterOptionForm = {
  code: string;
  is_archived: boolean;
  label: string;
  option_id?: string;
  sort_order: string;
};

type AssetParameterDimensionGroup = {
  dimension: string | null;
  label: string;
  parameters: AssetParameter[];
};

const DATA_TYPE_OPTIONS = [
  { label: "文本", value: "text" },
  { label: "数字", value: "number" },
  { label: "范围", value: "range" },
  { label: "布尔", value: "boolean" },
  { label: "日期", value: "date" },
  { label: "枚举", value: "enum" },
];

const NO_UNIT_DIMENSION_VALUE = "__none__";
const UNIT_DIMENSION_SELECT_OPTIONS = [
  { label: "无单位维度", value: NO_UNIT_DIMENSION_VALUE },
  ...UNIT_DIMENSION_OPTIONS,
];
const EMPTY_PARAMETERS: AssetParameter[] = [];
const EMPTY_UNITS: Unit[] = [];

export function AssetParametersPage() {
  const { currentUser } = useAuth();
  const {
    canManageSelectedLaboratoryAssets,
    selectedLaboratoryId,
    selectedLaboratoryName,
  } = useLaboratorySelection();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const canManage = canManageAssetParameters(currentUser);
  const parametersQuery = useAssetParameters({
    enabled: canManageSelectedLaboratoryAssets && Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const unitsQuery = useUnits();
  const createParameter = useCreateAssetParameter();
  const updateParameter = useUpdateAssetParameter();
  const deleteParameter = useDeleteAssetParameter();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<AssetParameter | "new" | null>(null);
  const parameters = parametersQuery.data ?? EMPTY_PARAMETERS;
  const units = unitsQuery.data ?? EMPTY_UNITS;
  const unitsById = useMemo(() => buildUnitIndex(units), [units]);
  const filteredParameters = useMemo(
    () => filterParameters(parameters, search, unitsById),
    [parameters, search, unitsById],
  );
  const parameterGroups = useMemo(
    () => groupParametersByUnitDimension(filteredParameters),
    [filteredParameters],
  );

  function refresh() {
    if (!selectedLaboratoryId) return;
    queryClient.invalidateQueries({
      queryKey: adminQueryKeys.assetParameters(apiBaseUrl, selectedLaboratoryId),
    });
  }

  function handleDelete(parameter: AssetParameter) {
    deleteParameter.mutate(parameter.parameter_type_id, {
      onError: (error) =>
        toast.error({ title: "删除资产参数失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "资产参数已删除" });
      },
    });
  }

  const columns: DataTableColumn<AssetParameter>[] = [
    {
      header: "参数",
      key: "name",
      render: (item) => (
        <span className="unit-name-cell">
          <SlidersHorizontal size={15} aria-hidden="true" />
          <strong>{item.name}</strong>
        </span>
      ),
    },
    {
      header: "代码",
      key: "code",
      render: (item) => <Badge>{item.code}</Badge>,
    },
    {
      header: "类型",
      key: "data-type",
      render: (item) => <Badge tone={dataTypeTone(item.data_type)}>{dataTypeLabel(item.data_type)}</Badge>,
    },
    {
      header: "默认单位",
      key: "default-unit",
      render: (item) => defaultUnitLabel(item.default_unit_id, unitsById),
    },
    {
      header: "状态",
      key: "status",
      render: (item) => (
        <Badge tone={item.is_archived ? "warning" : "success"}>
          {item.is_archived ? "已归档" : "启用"}
        </Badge>
      ),
    },
    { header: "更新时间", key: "updated", render: (item) => formatDate(item.updated_at) },
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
            description={`确认删除资产参数“${item.name}”？已经被资产记录引用的参数不能删除。`}
            disabled={!canManage || deleteParameter.isPending}
            title="删除资产参数"
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

  const pageActions = (
    <Button
      disabled={!canManageSelectedLaboratoryAssets || !selectedLaboratoryId}
      onClick={() => setEditing("new")}
      variant="primary"
    >
      <Plus size={15} />
      新建参数
    </Button>
  );

  if (!canManage) {
    return (
      <main className="page">
        <PageHeader
          kicker="管理"
          title="资产参数"
          description="当前账号没有管理资产参数的权限。"
        />
        <section className="panel">
          <EmptyState description="请切换到有权限的账号。" title="权限不足" />
        </section>
      </main>
    );
  }

  const loading = parametersQuery.isLoading || unitsQuery.isLoading;

  return (
    <main className="page">
      <PageHeader
        kicker="管理"
        title="资产参数"
        description="按实验室维护资产参数，并通过单位维度组织数字和范围参数。"
        actions={pageActions}
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">{selectedLaboratoryName || "未选择实验室"}</h2>
            <p className="panel-description">
              {!selectedLaboratoryId
                ? "请选择实验室后管理参数"
                : canManageSelectedLaboratoryAssets
                  ? `${filteredParameters.length} 个参数`
                  : "当前账号不能管理该实验室"}
            </p>
          </div>
          <input
            aria-label="搜索资产参数"
            className="input"
            placeholder="搜索参数..."
            style={{ maxWidth: 260 }}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        {loading ? (
          <DataTable
            columns={columns}
            emptyDescription="当前实验室还没有资产参数。"
            getRowKey={(item) => item.parameter_type_id}
            items={[]}
            loading
          />
        ) : filteredParameters.length === 0 ? (
          <EmptyState
            description={search ? "没有匹配的资产参数。" : "当前实验室还没有资产参数。"}
            title={search ? "未找到参数" : "暂无资产参数"}
          />
        ) : (
          <AssetParameterGroupsTable columns={columns} groups={parameterGroups} />
        )}
      </section>
      <AssetParameterEditor
        createParameter={createParameter}
        laboratoryId={selectedLaboratoryId}
        open={editing !== null}
        parameter={editing}
        units={units}
        updateParameter={updateParameter}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          refresh();
        }}
      />
    </main>
  );
}

function AssetParameterGroupsTable({
  columns,
  groups,
}: {
  columns: DataTableColumn<AssetParameter>[];
  groups: AssetParameterDimensionGroup[];
}) {
  return (
    <div className="table-wrap">
      <table className="data-table unit-groups-table asset-parameter-groups-table">
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.key}
                style={{ textAlign: column.align === "right" ? "right" : "left" }}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {groups.map((group) => {
            const headingId = parameterDimensionHeadingId(group.dimension);

            return (
              <Fragment key={group.dimension ?? NO_UNIT_DIMENSION_VALUE}>
                <tr className="unit-dimension-row">
                  <td colSpan={columns.length}>
                    <div className="unit-dimension-row-content">
                      <div>
                        <h3 className="unit-dimension-title" id={headingId}>
                          {group.label}
                        </h3>
                        <p className="unit-dimension-meta">
                          {group.dimension
                            ? `维度代码：${group.dimension}`
                            : "维度代码：未设置"}
                        </p>
                      </div>
                      <Badge tone="accent">{group.parameters.length} 个参数</Badge>
                    </div>
                  </td>
                </tr>
                {group.parameters.map((parameter) => (
                  <tr aria-describedby={headingId} key={parameter.parameter_type_id}>
                    {columns.map((column) => (
                      <td
                        key={column.key}
                        style={{ textAlign: column.align === "right" ? "right" : "left" }}
                      >
                        {column.render(parameter)}
                      </td>
                    ))}
                  </tr>
                ))}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function AssetParameterEditor({
  createParameter,
  laboratoryId,
  onClose,
  onSaved,
  open,
  parameter,
  units,
  updateParameter,
}: {
  createParameter: ReturnType<typeof useCreateAssetParameter>;
  laboratoryId: string;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  parameter: AssetParameter | "new" | null;
  units: Unit[];
  updateParameter: ReturnType<typeof useUpdateAssetParameter>;
}) {
  const toast = useToast();
  const isNew = parameter === "new";
  const editingParameter = parameter && parameter !== "new" ? parameter : null;
  const [values, setValues] = useState<ParameterForm>(emptyParameterForm());
  const isSaving = createParameter.isPending || updateParameter.isPending;
  const selectableUnits = useMemo(
    () =>
      values.unit_dimension
        ? units.filter((unit) => unit.dimension === values.unit_dimension)
        : units,
    [units, values.unit_dimension],
  );

  useEffect(() => {
    if (!parameter || parameter === "new") {
      setValues(emptyParameterForm());
      return;
    }

    setValues({
      code: parameter.code,
      data_type: parameter.data_type,
      default_unit_id: parameter.default_unit_id ?? "",
      description: parameter.description ?? "",
      is_archived: parameter.is_archived,
      name: parameter.name,
      options:
        parameter.options.length > 0
          ? parameter.options.map((option) => ({
              code: option.code,
              is_archived: option.is_archived,
              label: option.label,
              option_id: option.option_id,
              sort_order: String(option.sort_order),
            }))
          : [emptyOptionForm()],
      unit_dimension: parameter.unit_dimension ?? "",
    });
  }, [parameter]);

  function updateField<K extends keyof Omit<ParameterForm, "options">>(
    field: K,
    value: ParameterForm[K],
  ) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function updateDataType(value: string) {
    const dataType = value as AssetParameter["data_type"];
    const supportsUnits = dataTypeSupportsUnits(dataType);
    setValues((current) => ({
      ...current,
      data_type: dataType,
      default_unit_id: supportsUnits ? current.default_unit_id : "",
      options:
        dataType === "enum" && current.options.length === 0
          ? [emptyOptionForm()]
          : current.options,
      unit_dimension:
        supportsUnits ? current.unit_dimension || DEFAULT_UNIT_DIMENSION : "",
    }));
  }

  function updateUnitDimension(value: string) {
    const nextDimension = value === NO_UNIT_DIMENSION_VALUE ? "" : value;
    setValues((current) => {
      const selectedUnit = units.find((unit) => unit.unit_id === current.default_unit_id);
      const defaultUnitStillMatches =
        nextDimension &&
        selectedUnit &&
        selectedUnit.dimension === nextDimension;

      return {
        ...current,
        default_unit_id: defaultUnitStillMatches ? current.default_unit_id : "",
        unit_dimension: nextDimension,
      };
    });
  }

  function updateDefaultUnit(unitId: string) {
    setValues((current) => {
      const selectedUnit = units.find((unit) => unit.unit_id === unitId);

      return {
        ...current,
        default_unit_id: unitId,
        unit_dimension: selectedUnit?.dimension ?? current.unit_dimension,
      };
    });
  }

  function updateOption(
    index: number,
    field: keyof ParameterOptionForm,
    value: string | boolean,
  ) {
    setValues((current) => ({
      ...current,
      options: current.options.map((option, optionIndex) =>
        optionIndex === index ? { ...option, [field]: value } : option,
      ),
    }));
  }

  function addOption() {
    setValues((current) => ({
      ...current,
      options: [...current.options, emptyOptionForm()],
    }));
  }

  function removeOption(index: number) {
    setValues((current) => {
      const options = current.options.filter((_, optionIndex) => optionIndex !== index);
      return {
        ...current,
        options: options.length > 0 ? options : [emptyOptionForm()],
      };
    });
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!laboratoryId) {
      toast.error({ title: "请先选择实验室" });
      return;
    }

    const optionsResult = normalizeOptions(values);
    if (!optionsResult.ok) {
      toast.error({ title: optionsResult.message });
      return;
    }

    const payload: AssetParameterPayload = {
      code: values.code.trim(),
      data_type: values.data_type,
      default_unit_id: dataTypeSupportsUnits(values.data_type)
        ? values.default_unit_id || null
        : null,
      description: optionalText(values.description),
      is_archived: values.is_archived,
      name: values.name.trim(),
      options: optionsResult.options,
      unit_dimension: dataTypeSupportsUnits(values.data_type)
        ? optionalText(values.unit_dimension)
        : null,
    };

    if (!payload.name || !payload.code) {
      toast.error({ title: "请填写参数名称和参数代码" });
      return;
    }

    if (isNew) {
      createParameter.mutate(
        { laboratoryId, payload },
        {
          onError: (error) =>
            toast.error({ title: "创建资产参数失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "资产参数已创建" });
            onSaved();
          },
        },
      );
      return;
    }

    if (editingParameter) {
      updateParameter.mutate(
        { parameterId: editingParameter.parameter_type_id, payload },
        {
          onError: (error) =>
            toast.error({ title: "更新资产参数失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "资产参数已更新" });
            onSaved();
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="数字和范围参数可设置单位维度和默认单位；枚举参数需要至少一个启用选项。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建资产参数" : "编辑资产参数"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="asset-parameter-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="asset-parameter-form" onSubmit={handleSubmit}>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="asset-parameter-name" label="参数名称">
            <input
              className="input"
              id="asset-parameter-name"
              value={values.name}
              onChange={(event) => updateField("name", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="asset-parameter-code" label="参数代码">
            <input
              className="input"
              id="asset-parameter-code"
              value={values.code}
              onChange={(event) => updateField("code", event.target.value)}
            />
          </FormField>
        </div>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="asset-parameter-data-type" label="数据类型">
            <Select
              id="asset-parameter-data-type"
              label="数据类型"
              options={DATA_TYPE_OPTIONS}
              value={values.data_type}
              onValueChange={updateDataType}
            />
          </FormField>
          {dataTypeSupportsUnits(values.data_type) ? (
            <FormField htmlFor="asset-parameter-unit-dimension" label="单位维度">
              <Select
                id="asset-parameter-unit-dimension"
                label="单位维度"
                options={UNIT_DIMENSION_SELECT_OPTIONS}
                value={values.unit_dimension || NO_UNIT_DIMENSION_VALUE}
                onValueChange={updateUnitDimension}
              />
            </FormField>
          ) : null}
        </div>
        {dataTypeSupportsUnits(values.data_type) ? (
          <FormField htmlFor="asset-parameter-default-unit" label="默认单位">
            <select
              className="input"
              id="asset-parameter-default-unit"
              value={values.default_unit_id}
              onChange={(event) => updateDefaultUnit(event.target.value)}
            >
              <option value="">无默认单位</option>
              {selectableUnits.map((unit) => (
                <option key={unit.unit_id} value={unit.unit_id}>
                  {unit.name}（{unit.symbol}）
                </option>
              ))}
            </select>
          </FormField>
        ) : null}
        {values.data_type === "enum" ? (
          <ParameterOptionsEditor
            options={values.options}
            onAdd={addOption}
            onRemove={removeOption}
            onUpdate={updateOption}
          />
        ) : null}
        <FormField htmlFor="asset-parameter-description" label="描述">
          <textarea
            className="textarea"
            id="asset-parameter-description"
            value={values.description}
            onChange={(event) => updateField("description", event.target.value)}
          />
        </FormField>
        <label className="checkbox-field" htmlFor="asset-parameter-archived">
          <input
            checked={values.is_archived}
            id="asset-parameter-archived"
            type="checkbox"
            onChange={(event) => updateField("is_archived", event.target.checked)}
          />
          <span>
            <strong>归档参数</strong>
            <small>归档后参数仍会保留历史记录，但不再作为新资产的优先选择。</small>
          </span>
        </label>
      </form>
    </Dialog>
  );
}

function ParameterOptionsEditor({
  onAdd,
  onRemove,
  onUpdate,
  options,
}: {
  onAdd: () => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, field: keyof ParameterOptionForm, value: string | boolean) => void;
  options: ParameterOptionForm[];
}) {
  return (
    <div className="parameter-options-field">
      <div className="parameter-options-header">
        <div>
          <h3>枚举选项</h3>
          <p>至少保留一个未归档选项。</p>
        </div>
        <Button type="button" onClick={onAdd}>
          <Plus size={15} />
          添加选项
        </Button>
      </div>
      <div className="parameter-options-list">
        {options.map((option, index) => (
          <div className="parameter-option-row" key={option.option_id ?? index}>
            <FormField htmlFor={`asset-parameter-option-label-${index}`} label="显示名称">
              <input
                className="input"
                id={`asset-parameter-option-label-${index}`}
                value={option.label}
                onChange={(event) => onUpdate(index, "label", event.target.value)}
              />
            </FormField>
            <FormField htmlFor={`asset-parameter-option-code-${index}`} label="代码">
              <input
                className="input"
                id={`asset-parameter-option-code-${index}`}
                value={option.code}
                onChange={(event) => onUpdate(index, "code", event.target.value)}
              />
            </FormField>
            <FormField htmlFor={`asset-parameter-option-sort-${index}`} label="排序">
              <input
                className="input"
                id={`asset-parameter-option-sort-${index}`}
                type="number"
                value={option.sort_order}
                onChange={(event) => onUpdate(index, "sort_order", event.target.value)}
              />
            </FormField>
            <label
              className="checkbox-field parameter-option-archived"
              htmlFor={`asset-parameter-option-archived-${index}`}
            >
              <input
                checked={option.is_archived}
                id={`asset-parameter-option-archived-${index}`}
                type="checkbox"
                onChange={(event) => onUpdate(index, "is_archived", event.target.checked)}
              />
              <span>
                <strong>归档</strong>
              </span>
            </label>
            <Button
              aria-label="删除选项"
              className="parameter-option-delete"
              size="icon"
              type="button"
              variant="ghost"
              onClick={() => onRemove(index)}
            >
              <Trash2 size={15} />
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}

function emptyParameterForm(): ParameterForm {
  return {
    code: "",
    data_type: "number",
    default_unit_id: "",
    description: "",
    is_archived: false,
    name: "",
    options: [emptyOptionForm()],
    unit_dimension: DEFAULT_UNIT_DIMENSION,
  };
}

function emptyOptionForm(): ParameterOptionForm {
  return {
    code: "",
    is_archived: false,
    label: "",
    sort_order: "0",
  };
}

function normalizeOptions(values: ParameterForm):
  | { ok: true; options: AssetParameterOptionPayload[] }
  | { ok: false; message: string } {
  if (values.data_type !== "enum") {
    return { ok: true, options: [] };
  }

  const options = values.options
    .filter((option) => option.code.trim() || option.label.trim())
    .map((option) => {
      const sortOrder = Number(option.sort_order);
      return {
        code: option.code.trim(),
        is_archived: option.is_archived,
        label: option.label.trim(),
        option_id: option.option_id,
        sort_order: sortOrder,
      };
    });

  if (options.length === 0) {
    return { ok: false, message: "枚举参数至少需要一个选项" };
  }

  if (options.some((option) => !option.code || !option.label)) {
    return { ok: false, message: "请填写枚举选项的显示名称和代码" };
  }

  if (options.some((option) => !Number.isFinite(option.sort_order))) {
    return { ok: false, message: "枚举选项排序必须是数字" };
  }

  if (!options.some((option) => !option.is_archived)) {
    return { ok: false, message: "枚举参数至少需要一个启用选项" };
  }

  return { ok: true, options };
}

function buildUnitIndex(units: Unit[]) {
  return new Map(units.map((unit) => [unit.unit_id, unit]));
}

function filterParameters(
  parameters: AssetParameter[],
  search: string,
  unitsById: Map<string, Unit>,
) {
  const normalizedSearch = search.trim().toLowerCase();
  if (!normalizedSearch) {
    return parameters;
  }

  return parameters.filter((parameter) => {
    const defaultUnit = parameter.default_unit_id
      ? unitsById.get(parameter.default_unit_id)
      : null;

    return [
      parameter.code,
      parameter.name,
      parameter.description ?? "",
      parameter.data_type,
      dataTypeLabel(parameter.data_type),
      parameter.unit_dimension ?? "",
      parameter.unit_dimension ? unitDimensionLabel(parameter.unit_dimension) : "无单位维度",
      defaultUnit?.name ?? "",
      defaultUnit?.symbol ?? "",
      parameter.options.map((option) => `${option.label} ${option.code}`).join(" "),
    ]
      .join(" ")
      .toLowerCase()
      .includes(normalizedSearch);
  });
}

function groupParametersByUnitDimension(
  parameters: AssetParameter[],
): AssetParameterDimensionGroup[] {
  const groups = new Map<string, AssetParameter[]>();

  for (const parameter of parameters) {
    const dimension = parameter.unit_dimension ?? "";
    const group = groups.get(dimension);

    if (group) {
      group.push(parameter);
      continue;
    }

    groups.set(dimension, [parameter]);
  }

  const orderedGroups: AssetParameterDimensionGroup[] = [];

  for (const option of UNIT_DIMENSION_OPTIONS) {
    const group = groups.get(option.value);

    if (!group) continue;

    orderedGroups.push({
      dimension: option.value,
      label: option.label,
      parameters: group,
    });
    groups.delete(option.value);
  }

  const unclassified = groups.get("") ?? null;
  groups.delete("");

  const remainingGroups: AssetParameterDimensionGroup[] = Array.from(groups.entries())
    .map(([dimension, group]) => ({
      dimension,
      label: unitDimensionLabel(dimension),
      parameters: group,
    }))
    .sort((left, right) => left.label.localeCompare(right.label, "zh-CN"));

  if (unclassified) {
    remainingGroups.push({
      dimension: null,
      label: "无单位维度",
      parameters: unclassified,
    });
  }

  return [...orderedGroups, ...remainingGroups];
}

function parameterDimensionHeadingId(dimension: string | null) {
  return `asset-parameter-dimension-${(dimension ?? "none").replace(/[^a-z0-9_-]+/gi, "-")}`;
}

function dataTypeLabel(dataType: AssetParameter["data_type"]) {
  const option = DATA_TYPE_OPTIONS.find((item) => item.value === dataType);
  return option?.label ?? dataType;
}

function dataTypeTone(dataType: AssetParameter["data_type"]) {
  if (dataType === "number") return "accent";
  if (dataType === "range") return "accent";
  if (dataType === "enum") return "warning";
  if (dataType === "boolean") return "success";
  return "default";
}

function dataTypeSupportsUnits(dataType: AssetParameter["data_type"]) {
  return dataType === "number" || dataType === "range";
}

function defaultUnitLabel(unitId: string | null, unitsById: Map<string, Unit>) {
  if (!unitId) {
    return "未设置";
  }

  const unit = unitsById.get(unitId);
  return unit ? `${unit.name}（${unit.symbol}）` : unitId;
}
