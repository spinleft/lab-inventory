import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronLeft,
  ChevronRight,
  Filter,
  MoreHorizontal,
  Pencil,
  Plus,
  RotateCcw,
  Settings2,
  Trash2,
} from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { Dialog } from "../../shared/ui/Dialog";
import { EmptyState } from "../../shared/ui/EmptyState";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import {
  type AssetCategory,
  type AssetParameter,
  type Location,
  type Unit,
  adminQueryKeys,
  optionalText,
  useAssetCategories,
  useAssetParameters,
  useLocations,
  useUnits,
} from "../admin/api";
import {
  type PendingAttachment,
  PendingAttachmentUploader,
  attachmentClaimsFromPending,
} from "../attachments/AttachmentPanel";
import {
  type Asset,
  type AssetParameterPayloadValue,
  type AssetParameterValue,
  type AssetParameterValuePayload,
  type AssetPayload,
  type AssetQuery,
  type AssetTrackingMode,
  assetQueryKeys,
  useAssets,
  useCreateAsset,
  useDeleteAsset,
  useUpdateAsset,
} from "./api";

type ViewMode = "basic" | "parameters";

type FilterForm = {
  category_id: string;
  exact_category: boolean;
  has_inventory: "all" | "true" | "false";
  inventory_status: string;
  is_archived: "all" | "true" | "false";
  keyword: string;
  location_id: string;
  manufacturer: string;
  parameter_keyword: string;
  parameter_type_id: string;
  tracking_mode: "all" | AssetTrackingMode;
};

type SortState = {
  direction: "asc" | "desc";
  key: string;
};

type AssetColumn = {
  align?: "left" | "right";
  key: string;
  label: string;
  locked?: boolean;
  render: (asset: Asset) => ReactNode;
  sortKey?: string;
};

type AssetEditorMode = Asset | "new" | null;

type AssetForm = {
  category_id: string;
  default_unit_id: string;
  internal_notes: string;
  is_archived: boolean;
  manufacturer: string;
  model: string;
  name: string;
  public_notes: string;
  tracking_mode: AssetTrackingMode;
};

type ParameterInput = {
  boolean: "" | "true" | "false";
  date: string;
  number: string;
  option_id: string;
  range_end: string;
  range_start: string;
  text: string;
  unit_id: string;
};

type ParameterPayloadResult =
  | { ok: true; values: AssetParameterValuePayload[] }
  | { message: string; ok: false };

const PAGE_SIZE = 30;
const EMPTY_CATEGORIES: AssetCategory[] = [];
const EMPTY_PARAMETERS: AssetParameter[] = [];
const EMPTY_LOCATIONS: Location[] = [];
const EMPTY_UNITS: Unit[] = [];
const DEFAULT_BASIC_COLUMNS = [
  "asset",
  "category",
  "tracking_mode",
  "manufacturer",
  "inventory",
  "archived",
  "updated_at",
];
const DEFAULT_PARAMETER_BASE_COLUMNS = ["asset", "category", "updated_at"];

export function AssetsPage() {
  const {
    canManageSelectedLaboratoryAssets,
    selectedLaboratoryId,
    selectedLaboratoryName,
  } = useLaboratorySelection();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const canManage = canManageSelectedLaboratoryAssets;
  const categoryFromUrl = searchParams.get("category_id") ?? "";
  const exactCategoryFromUrl = searchParams.get("exact_category") === "true";
  const locationFromUrl = searchParams.get("location_id") ?? "";
  const [offset, setOffset] = useState(0);
  const [viewMode, setViewMode] = useState<ViewMode>("basic");
  const [filters, setFilters] = useState<FilterForm>(() =>
    emptyFilters(categoryFromUrl, exactCategoryFromUrl, locationFromUrl),
  );
  const [draftFilters, setDraftFilters] = useState<FilterForm>(() =>
    emptyFilters(categoryFromUrl, exactCategoryFromUrl, locationFromUrl),
  );
  const [sort, setSort] = useState<SortState>({ direction: "asc", key: "name" });
  const [visibleBasicColumns, setVisibleBasicColumns] = useState<Set<string>>(
    () => new Set(DEFAULT_BASIC_COLUMNS),
  );
  const [visibleParameterColumns, setVisibleParameterColumns] = useState<Set<string>>(
    () => new Set(DEFAULT_PARAMETER_BASE_COLUMNS),
  );
  const [editing, setEditing] = useState<AssetEditorMode>(null);
  const [deletingAsset, setDeletingAsset] = useState<Asset | null>(null);

  const categoriesQuery = useAssetCategories({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const parametersQuery = useAssetParameters({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const locationsQuery = useLocations({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const unitsQuery = useUnits();
  const createAsset = useCreateAsset();
  const updateAsset = useUpdateAsset();
  const deleteAsset = useDeleteAsset();

  const categories = categoriesQuery.data ?? EMPTY_CATEGORIES;
  const parameters = parametersQuery.data ?? EMPTY_PARAMETERS;
  const locations = locationsQuery.data ?? EMPTY_LOCATIONS;
  const units = unitsQuery.data ?? EMPTY_UNITS;
  const categoryById = useMemo(() => mapById(categories, "category_id"), [categories]);
  const unitsById = useMemo(() => mapById(units, "unit_id"), [units]);
  useEffect(() => {
    setFilters((current) => ({
      ...current,
      category_id: categoryFromUrl,
      exact_category: exactCategoryFromUrl,
      location_id: locationFromUrl,
    }));
    setDraftFilters((current) => ({
      ...current,
      category_id: categoryFromUrl,
      exact_category: exactCategoryFromUrl,
      location_id: locationFromUrl,
    }));
    setOffset(0);
  }, [categoryFromUrl, exactCategoryFromUrl, locationFromUrl]);

  useEffect(() => {
    setOffset(0);
  }, [selectedLaboratoryId]);

  useEffect(() => {
    const parameterKeys = parameters.map((parameter) => parameterColumnKey(parameter));
    setVisibleParameterColumns((current) => {
      const validKeys = new Set([...DEFAULT_PARAMETER_BASE_COLUMNS, ...parameterKeys]);
      const next = new Set([...current].filter((key) => validKeys.has(key)));
      for (const key of DEFAULT_PARAMETER_BASE_COLUMNS) {
        next.add(key);
      }
      if (![...next].some((key) => key.startsWith("param:"))) {
        for (const key of parameterKeys.slice(0, 5)) {
          next.add(key);
        }
      }
      return next;
    });
  }, [parameters]);

  const query = useMemo<AssetQuery>(
    () => ({
      category_id: optional(filters.category_id),
      exact_category: filters.exact_category || undefined,
      has_inventory: optionalBoolean(filters.has_inventory),
      include: "parameters",
      inventory_status: optional(filters.inventory_status),
      is_archived: optionalBoolean(filters.is_archived),
      keyword: optional(filters.keyword),
      limit: PAGE_SIZE,
      location_id: optional(filters.location_id),
      manufacturer: optional(filters.manufacturer),
      offset,
      tracking_mode:
        filters.tracking_mode === "all" ? undefined : filters.tracking_mode,
    }),
    [filters, offset],
  );
  const assetsQuery = useAssets({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    query,
  });
  const response = assetsQuery.data;
  const total = response?.total ?? 0;
  const page = Math.floor(offset / PAGE_SIZE) + 1;
  const maxPage = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const parameterFilteredAssets = useMemo(
    () =>
      (response?.items ?? []).filter((asset) =>
        assetMatchesParameterFilters(asset, filters, unitsById),
      ),
    [filters, response?.items, unitsById],
  );
  const visibleAssets = useMemo(
    () => sortAssets(parameterFilteredAssets, sort, categoryById, unitsById),
    [categoryById, parameterFilteredAssets, sort, unitsById],
  );
  const parameterFilterActive = Boolean(
    filters.parameter_type_id || filters.parameter_keyword.trim(),
  );
  const categoryBreadcrumbs = useMemo(
    () => buildCategoryBreadcrumbs(filters.category_id, categories),
    [categories, filters.category_id],
  );
  const basicColumns = useMemo(
    () => buildBasicColumns(categoryById, unitsById),
    [categoryById, unitsById],
  );
  const parameterColumns = useMemo(
    () => buildParameterColumns(parameters, categoryById, unitsById),
    [categoryById, parameters, unitsById],
  );
  const activeColumns = viewMode === "basic" ? basicColumns : parameterColumns;
  const visibleColumnKeys =
    viewMode === "basic" ? visibleBasicColumns : visibleParameterColumns;
  const visibleColumns = activeColumns.filter(
    (column) => column.locked || visibleColumnKeys.has(column.key),
  );

  function refreshAssets() {
    queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
    if (selectedLaboratoryId) {
      queryClient.invalidateQueries({
        queryKey: adminQueryKeys.assetCategories(apiBaseUrl, selectedLaboratoryId),
      });
      queryClient.invalidateQueries({
        queryKey: adminQueryKeys.assetParameters(apiBaseUrl, selectedLaboratoryId),
      });
    }
  }

  function submitFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setOffset(0);
    setFilters(draftFilters);
    syncAssetSearch(
      draftFilters.category_id,
      draftFilters.exact_category,
      draftFilters.location_id,
    );
  }

  function resetFilters() {
    const next = emptyFilters();
    setDraftFilters(next);
    setFilters(next);
    setOffset(0);
    setSearchParams({});
  }

  function applyCategory(categoryId: string, exactCategory = false) {
    setOffset(0);
    setDraftFilters((current) => ({
      ...current,
      category_id: categoryId,
      exact_category: exactCategory,
    }));
    setFilters((current) => ({
      ...current,
      category_id: categoryId,
      exact_category: exactCategory,
    }));
    syncCategorySearch(categoryId, exactCategory);
  }

  function syncCategorySearch(categoryId: string, exactCategory: boolean) {
    syncAssetSearch(categoryId, exactCategory, filters.location_id);
  }

  function syncAssetSearch(categoryId: string, exactCategory: boolean, locationId: string) {
    const next = new URLSearchParams(searchParams);
    if (categoryId) {
      next.set("category_id", categoryId);
    } else {
      next.delete("category_id");
    }
    if (categoryId && exactCategory) {
      next.set("exact_category", "true");
    } else {
      next.delete("exact_category");
    }
    if (locationId) {
      next.set("location_id", locationId);
    } else {
      next.delete("location_id");
    }
    setSearchParams(next, { replace: true });
  }

  function updateDraftFilter<K extends keyof FilterForm>(key: K, value: FilterForm[K]) {
    setDraftFilters((current) => ({ ...current, [key]: value }));
  }

  function handleSort(key: string) {
    setSort((current) =>
      current.key === key
        ? { direction: current.direction === "asc" ? "desc" : "asc", key }
        : { direction: "asc", key },
    );
  }

  function toggleColumn(key: string, visible: boolean) {
    const setter =
      viewMode === "basic" ? setVisibleBasicColumns : setVisibleParameterColumns;
    setter((current) => {
      const next = new Set(current);
      if (visible) {
        next.add(key);
      } else {
        next.delete(key);
      }
      return next;
    });
  }

  function confirmDelete() {
    if (!deletingAsset) return;
    deleteAsset.mutate(deletingAsset.asset_id, {
      onError: (error) =>
        toast.error({ title: "删除资产失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        toast.success({ title: "资产已删除" });
        setDeletingAsset(null);
        refreshAssets();
      },
    });
  }

  const pageActions = (
    <Button
      disabled={!canManage || !selectedLaboratoryId}
      onClick={() => setEditing("new")}
      variant="primary"
    >
      <Plus size={15} />
      添加资产
    </Button>
  );

  return (
    <main className="page">
      <PageHeader
        kicker="资产"
        title="资产"
        description="查看资产列表，并按分类、属性、库存和参数缩小查询范围。"
        actions={pageActions}
      />

      <CategoryPathNav breadcrumbs={categoryBreadcrumbs} onSelect={applyCategory} />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">查询条件</h2>
            <p className="panel-description">
              {selectedLaboratoryName || "请选择实验室"} · 分类默认包含子分类
            </p>
          </div>
        </div>
        <div className="panel-body">
          <form className="asset-filter-form" onSubmit={submitFilters}>
            <div className="asset-filter-grid">
              <FormField htmlFor="asset-keyword" label="关键词">
                <input
                  className="input"
                  id="asset-keyword"
                  placeholder="名称、型号、厂商或备注"
                  value={draftFilters.keyword}
                  onChange={(event) => updateDraftFilter("keyword", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="asset-category" label="资产分类">
                <Select
                  id="asset-category"
                  label="资产分类"
                  options={[
                    { label: "全部分类", value: "all" },
                    ...categories.map((category) => ({
                      label: `${"　".repeat(category.depth)}${category.name}`,
                      value: category.category_id,
                    })),
                  ]}
                  value={draftFilters.category_id || "all"}
                  onValueChange={(value) =>
                    updateDraftFilter("category_id", value === "all" ? "" : value)
                  }
                />
              </FormField>
              <FormField htmlFor="asset-tracking-mode" label="管理模式">
                <Select
                  id="asset-tracking-mode"
                  label="管理模式"
                  options={[
                    { label: "全部模式", value: "all" },
                    { label: "序列号管理", value: "serialized" },
                    { label: "数量管理", value: "quantity" },
                  ]}
                  value={draftFilters.tracking_mode}
                  onValueChange={(value) =>
                    updateDraftFilter("tracking_mode", value as FilterForm["tracking_mode"])
                  }
                />
              </FormField>
              <FormField htmlFor="asset-archived" label="归档状态">
                <Select
                  id="asset-archived"
                  label="归档状态"
                  options={[
                    { label: "全部状态", value: "all" },
                    { label: "未归档", value: "false" },
                    { label: "已归档", value: "true" },
                  ]}
                  value={draftFilters.is_archived}
                  onValueChange={(value) =>
                    updateDraftFilter("is_archived", value as FilterForm["is_archived"])
                  }
                />
              </FormField>
              <FormField htmlFor="asset-manufacturer" label="厂商">
                <input
                  className="input"
                  id="asset-manufacturer"
                  value={draftFilters.manufacturer}
                  onChange={(event) => updateDraftFilter("manufacturer", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="asset-inventory-status" label="库存状态">
                <Select
                  id="asset-inventory-status"
                  label="库存状态"
                  options={[
                    { label: "全部库存状态", value: "all" },
                    { label: "可用", value: "available" },
                    { label: "预留", value: "reserved" },
                    { label: "借出", value: "checked_out" },
                    { label: "维护中", value: "maintenance" },
                    { label: "退役", value: "retired" },
                  ]}
                  value={draftFilters.inventory_status || "all"}
                  onValueChange={(value) =>
                    updateDraftFilter("inventory_status", value === "all" ? "" : value)
                  }
                />
              </FormField>
              <FormField htmlFor="asset-location" label="位置">
                <Select
                  id="asset-location"
                  label="位置"
                  options={[
                    { label: "全部位置", value: "all" },
                    ...locations.map((location) => ({
                      label: `${"　".repeat(location.depth)}${location.name}`,
                      value: location.location_id,
                    })),
                  ]}
                  value={draftFilters.location_id || "all"}
                  onValueChange={(value) =>
                    updateDraftFilter("location_id", value === "all" ? "" : value)
                  }
                />
              </FormField>
              <FormField htmlFor="asset-has-inventory" label="是否有库存">
                <Select
                  id="asset-has-inventory"
                  label="是否有库存"
                  options={[
                    { label: "全部", value: "all" },
                    { label: "有库存项", value: "true" },
                    { label: "无库存项", value: "false" },
                  ]}
                  value={draftFilters.has_inventory}
                  onValueChange={(value) =>
                    updateDraftFilter("has_inventory", value as FilterForm["has_inventory"])
                  }
                />
              </FormField>
              <FormField htmlFor="asset-parameter-type" label="参数类型">
                <Select
                  id="asset-parameter-type"
                  label="参数类型"
                  options={[
                    { label: "全部参数", value: "all" },
                    ...parameters.map((parameter) => ({
                      label: `${parameter.name} (${parameter.code})`,
                      value: parameter.parameter_type_id,
                    })),
                  ]}
                  value={draftFilters.parameter_type_id || "all"}
                  onValueChange={(value) =>
                    updateDraftFilter("parameter_type_id", value === "all" ? "" : value)
                  }
                />
              </FormField>
              <FormField htmlFor="asset-parameter-keyword" label="参数搜索">
                <input
                  className="input"
                  id="asset-parameter-keyword"
                  placeholder="参数名、代码或值"
                  value={draftFilters.parameter_keyword}
                  onChange={(event) =>
                    updateDraftFilter("parameter_keyword", event.target.value)
                  }
                />
              </FormField>
            </div>
            <label className="checkbox-field asset-filter-checkbox" htmlFor="asset-exact-category">
              <input
                checked={draftFilters.exact_category}
                disabled={!draftFilters.category_id}
                id="asset-exact-category"
                type="checkbox"
                onChange={(event) =>
                  updateDraftFilter("exact_category", event.target.checked)
                }
              />
              <span>
                <strong>精确分类</strong>
                <small>关闭时包含所选分类的所有子分类资产。</small>
              </span>
            </label>
            <div className="toolbar-group">
              <Button type="submit" variant="primary">
                <Filter size={15} />
                应用筛选
              </Button>
              <Button onClick={resetFilters}>
                <RotateCcw size={15} />
                重置
              </Button>
            </div>
          </form>
        </div>
      </section>

      <section className="panel">
        <div className="panel-header asset-table-header">
          <div>
            <h2 className="panel-title">资产列表</h2>
            <p className="panel-description">
              第 {page} / {maxPage} 页，共 {total} 条
              {parameterFilterActive ? `，当前页参数匹配 ${visibleAssets.length} 条` : ""}
            </p>
          </div>
          <div className="toolbar-group">
            <div className="tabs-list asset-view-tabs" role="tablist" aria-label="资产视图">
              <button
                className="tabs-trigger"
                data-state={viewMode === "basic" ? "active" : "inactive"}
                role="tab"
                type="button"
                onClick={() => setViewMode("basic")}
              >
                基本信息
              </button>
              <button
                className="tabs-trigger"
                data-state={viewMode === "parameters" ? "active" : "inactive"}
                role="tab"
                type="button"
                onClick={() => setViewMode("parameters")}
              >
                参数信息
              </button>
            </div>
            <ColumnSelector
              columns={activeColumns}
              visibleColumns={visibleColumnKeys}
              onToggle={toggleColumn}
            />
            <Button
              disabled={offset <= 0 || assetsQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="上一页"
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              <ChevronLeft size={16} />
            </Button>
            <Button
              disabled={offset + PAGE_SIZE >= total || assetsQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="下一页"
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              <ChevronRight size={16} />
            </Button>
          </div>
        </div>
        <AssetsTable
          canManage={canManage}
          columns={visibleColumns}
          items={visibleAssets}
          loading={
            assetsQuery.isLoading ||
            categoriesQuery.isLoading ||
            parametersQuery.isLoading ||
            unitsQuery.isLoading
          }
          sort={sort}
          onDelete={setDeletingAsset}
          onEdit={setEditing}
          onRowClick={(asset) => navigate(`/assets/${asset.asset_id}`)}
          onSort={handleSort}
        />
      </section>

      <AssetEditor
        asset={editing}
        categories={categories}
        createAsset={createAsset}
        laboratoryId={selectedLaboratoryId}
        open={editing !== null}
        parameters={parameters}
        units={units}
        updateAsset={updateAsset}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          refreshAssets();
        }}
      />
      <DeleteAssetDialog
        asset={deletingAsset}
        loading={deleteAsset.isPending}
        onClose={() => setDeletingAsset(null)}
        onConfirm={confirmDelete}
      />
    </main>
  );
}

function CategoryPathNav({
  breadcrumbs,
  onSelect,
}: {
  breadcrumbs: Array<{ categoryId: string; label: string }>;
  onSelect: (categoryId: string) => void;
}) {
  return (
    <nav className="asset-category-path" aria-label="资产分类路径">
      {breadcrumbs.map((breadcrumb, index) => (
        <span className="asset-category-path-item" key={breadcrumb.categoryId || "all"}>
          {index > 0 ? <span className="asset-category-path-separator">/</span> : null}
          <button type="button" onClick={() => onSelect(breadcrumb.categoryId)}>
            {breadcrumb.label}
          </button>
        </span>
      ))}
    </nav>
  );
}

function ColumnSelector({
  columns,
  onToggle,
  visibleColumns,
}: {
  columns: AssetColumn[];
  onToggle: (key: string, visible: boolean) => void;
  visibleColumns: Set<string>;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button aria-label="选择列" size="icon" variant="ghost">
          <Settings2 size={16} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="dropdown-content asset-column-menu" align="end">
          {columns.map((column) => (
            <DropdownMenu.CheckboxItem
              checked={column.locked || visibleColumns.has(column.key)}
              className="dropdown-item"
              disabled={column.locked}
              key={column.key}
              onCheckedChange={(checked) => onToggle(column.key, checked === true)}
            >
              {column.label}
            </DropdownMenu.CheckboxItem>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function AssetsTable({
  canManage,
  columns,
  items,
  loading,
  onDelete,
  onEdit,
  onRowClick,
  onSort,
  sort,
}: {
  canManage: boolean;
  columns: AssetColumn[];
  items: Asset[];
  loading: boolean;
  onDelete: (asset: Asset) => void;
  onEdit: (asset: Asset) => void;
  onRowClick: (asset: Asset) => void;
  onSort: (key: string) => void;
  sort: SortState;
}) {
  if (loading) {
    return (
      <div className="panel-body">
        <div className="skeleton" style={{ height: 260 }} />
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <EmptyState
        description="当前查询条件下没有资产。"
        title="暂无资产"
      />
    );
  }

  return (
    <div className="table-wrap">
      <table className="data-table asset-table">
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.key}
                style={{ textAlign: column.align === "right" ? "right" : "left" }}
              >
                {column.sortKey ? (
                  <button
                    className="table-sort-button"
                    type="button"
                    onClick={() => onSort(column.sortKey ?? column.key)}
                  >
                    {column.label}
                    {sort.key === column.sortKey ? (
                      sort.direction === "asc" ? (
                        <ArrowUp size={13} />
                      ) : (
                        <ArrowDown size={13} />
                      )
                    ) : (
                      <ArrowUpDown size={13} />
                    )}
                  </button>
                ) : (
                  column.label
                )}
              </th>
            ))}
            <th style={{ textAlign: "right" }}>操作</th>
          </tr>
        </thead>
        <tbody>
          {items.map((asset) => (
            <tr
              className="asset-clickable-row"
              key={asset.asset_id}
              tabIndex={0}
              onClick={() => onRowClick(asset)}
              onKeyDown={(event) => {
                if (event.key === "Enter") onRowClick(asset);
              }}
            >
              {columns.map((column) => (
                <td
                  key={column.key}
                  style={{ textAlign: column.align === "right" ? "right" : "left" }}
                >
                  {column.render(asset)}
                </td>
              ))}
              <td style={{ textAlign: "right" }} onClick={(event) => event.stopPropagation()}>
                <AssetActions
                  asset={asset}
                  canManage={canManage}
                  onDelete={onDelete}
                  onEdit={onEdit}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function AssetActions({
  asset,
  canManage,
  onDelete,
  onEdit,
}: {
  asset: Asset;
  canManage: boolean;
  onDelete: (asset: Asset) => void;
  onEdit: (asset: Asset) => void;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          aria-label={`资产 ${asset.name} 操作`}
          disabled={!canManage}
          size="icon"
          variant="ghost"
          onClick={(event) => event.stopPropagation()}
        >
          <MoreHorizontal size={16} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="dropdown-content" align="end">
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onEdit(asset)}>
            <Pencil size={15} />
            编辑资产
          </DropdownMenu.Item>
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onDelete(asset)}>
            <Trash2 size={15} />
            删除资产
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function AssetEditor({
  asset,
  categories,
  createAsset,
  laboratoryId,
  onClose,
  onSaved,
  open,
  parameters,
  units,
  updateAsset,
}: {
  asset: AssetEditorMode;
  categories: AssetCategory[];
  createAsset: ReturnType<typeof useCreateAsset>;
  laboratoryId: string;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  parameters: AssetParameter[];
  units: Unit[];
  updateAsset: ReturnType<typeof useUpdateAsset>;
}) {
  const toast = useToast();
  const isNew = asset === "new";
  const editingAsset = asset && asset !== "new" ? asset : null;
  const [values, setValues] = useState<AssetForm>(() => emptyAssetForm(units));
  const [parameterInputs, setParameterInputs] = useState<Record<string, ParameterInput>>({});
  const [extraParameterIds, setExtraParameterIds] = useState<Set<string>>(() => new Set());
  const [removedExtraParameterIds, setRemovedExtraParameterIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [nextExtraParameterId, setNextExtraParameterId] = useState("");
  const [pendingAttachments, setPendingAttachments] = useState<PendingAttachment[]>([]);
  const unitsById = useMemo(() => mapById(units, "unit_id"), [units]);
  const parameterById = useMemo(
    () => mapById(parameters, "parameter_type_id"),
    [parameters],
  );
  const categoryParameterIds = useMemo(
    () => parameterIdsForCategory(values.category_id, categories),
    [categories, values.category_id],
  );
  const existingParameterIds = useMemo(
    () => new Set((editingAsset?.parameters ?? []).map((value) => value.parameter_type_id)),
    [editingAsset?.parameters],
  );
  const visibleParameters = useMemo(
    () =>
      parametersForEditor({
        categoryParameterIds,
        extraParameterIds,
        parameterById,
        parameters,
      }),
    [categoryParameterIds, extraParameterIds, parameterById, parameters],
  );
  const availableExtraParameters = useMemo(
    () =>
      parameters.filter(
        (parameter) =>
          !parameter.is_archived &&
          !categoryParameterIds.has(parameter.parameter_type_id) &&
          !extraParameterIds.has(parameter.parameter_type_id),
      ),
    [categoryParameterIds, extraParameterIds, parameters],
  );
  const isSaving = createAsset.isPending || updateAsset.isPending;

  useEffect(() => {
    if (!asset || asset === "new") {
      setValues(emptyAssetForm(units));
      setParameterInputs(buildParameterInputs(parameters, units, null));
      setExtraParameterIds(new Set());
      setRemovedExtraParameterIds(new Set());
      setNextExtraParameterId("");
      setPendingAttachments([]);
      return;
    }

    setValues({
      category_id: asset.category_id ?? "",
      default_unit_id: asset.default_unit_id,
      internal_notes: asset.internal_notes ?? "",
      is_archived: asset.is_archived,
      manufacturer: asset.manufacturer ?? "",
      model: asset.model ?? "",
      name: asset.name,
      public_notes: asset.public_notes ?? "",
      tracking_mode: asset.tracking_mode,
    });
    setParameterInputs(buildParameterInputs(parameters, units, asset.parameters ?? []));
    setExtraParameterIds(extraParameterIdsFromValues(asset, categories, parameters));
    setRemovedExtraParameterIds(new Set());
    setNextExtraParameterId("");
    setPendingAttachments([]);
  }, [asset, categories, parameters, units]);

  function updateField<K extends keyof AssetForm>(key: K, value: AssetForm[K]) {
    setValues((current) => ({ ...current, [key]: value }));
  }

  function updateParameter(parameterId: string, value: Partial<ParameterInput>) {
    setParameterInputs((current) => ({
      ...current,
      [parameterId]: {
        ...(current[parameterId] ?? emptyParameterInput()),
        ...value,
      },
    }));
  }

  function addExtraParameter() {
    if (!nextExtraParameterId) {
      return;
    }

    setExtraParameterIds((current) => new Set(current).add(nextExtraParameterId));
    setRemovedExtraParameterIds((current) => {
      const next = new Set(current);
      next.delete(nextExtraParameterId);
      return next;
    });
    setParameterInputs((current) => {
      if (current[nextExtraParameterId]) {
        return current;
      }
      const parameter = parameterById.get(nextExtraParameterId);
      return {
        ...current,
        [nextExtraParameterId]: emptyParameterInput(
          parameter ? defaultUnitForParameter(parameter, units) : "",
        ),
      };
    });
    setNextExtraParameterId("");
  }

  function removeExtraParameter(parameterId: string) {
    setExtraParameterIds((current) => {
      const next = new Set(current);
      next.delete(parameterId);
      return next;
    });
    setRemovedExtraParameterIds((current) => {
      if (!existingParameterIds.has(parameterId)) {
        return current;
      }
      return new Set(current).add(parameterId);
    });
    setParameterInputs((current) => ({
      ...current,
      [parameterId]: emptyParameterInput(
        parameterById.has(parameterId)
          ? defaultUnitForParameter(parameterById.get(parameterId)!, units)
          : "",
      ),
    }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!laboratoryId) {
      toast.error({ title: "请先选择实验室" });
      return;
    }

    const name = values.name.trim();
    if (!name) {
      toast.error({ title: "请填写资产名称" });
      return;
    }
    if (!values.default_unit_id) {
      toast.error({ title: "请先选择默认单位" });
      return;
    }

    const serializedParameters = serializeParameterPayloads({
      existingValues: editingAsset?.parameters ?? [],
      inputs: parameterInputs,
      parameters: visibleParameters,
      removedParameterIds: removedExtraParameterIds,
    });
    if (!serializedParameters.ok) {
      toast.error({ title: serializedParameters.message });
      return;
    }
    const attachmentClaims = isNew
      ? attachmentClaimsFromPending(pendingAttachments)
      : attachmentClaimsFromPending([]);
    if (!attachmentClaims.ok) {
      toast.error({ title: attachmentClaims.message });
      return;
    }

    const payload: AssetPayload = {
      attachments: isNew && attachmentClaims.claims.length > 0 ? attachmentClaims.claims : undefined,
      category_id: values.category_id || null,
      default_unit_id: values.default_unit_id,
      internal_notes: optionalText(values.internal_notes),
      is_archived: values.is_archived,
      manufacturer: optionalText(values.manufacturer),
      model: optionalText(values.model),
      name,
      parameters: serializedParameters.values,
      public_notes: optionalText(values.public_notes),
      tracking_mode: values.tracking_mode,
    };

    if (isNew) {
      createAsset.mutate(
        {
          laboratoryId,
          payload: {
            ...payload,
            default_unit_id: values.default_unit_id,
            name,
            tracking_mode: values.tracking_mode,
          },
        },
        {
          onError: (error) =>
            toast.error({ title: "创建资产失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            setPendingAttachments([]);
            toast.success({ title: "资产已创建" });
            onSaved();
          },
        },
      );
      return;
    }

    if (editingAsset) {
      updateAsset.mutate(
        { assetId: editingAsset.asset_id, payload },
        {
          onError: (error) =>
            toast.error({ title: "更新资产失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "资产已更新" });
            onSaved();
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="保存基础信息和参数值；库存项的独立调整留在库存流程中处理。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "添加资产" : "编辑资产"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="asset-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="asset-form" onSubmit={handleSubmit}>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="asset-name" label="资产名称">
            <input
              className="input"
              id="asset-name"
              value={values.name}
              onChange={(event) => updateField("name", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="asset-editor-category" label="资产分类">
            <Select
              id="asset-editor-category"
              label="资产分类"
              options={[
                { label: "无分类", value: "none" },
                ...categories.map((category) => ({
                  label: `${"　".repeat(category.depth)}${category.name}`,
                  value: category.category_id,
                })),
              ]}
              value={values.category_id || "none"}
              onValueChange={(value) =>
                updateField("category_id", value === "none" ? "" : value)
              }
            />
          </FormField>
        </div>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="asset-editor-mode" label="管理模式">
            <Select
              disabled={!isNew && (editingAsset?.inventory_summary.item_count ?? 0) > 0}
              id="asset-editor-mode"
              label="管理模式"
              options={[
                { label: "序列号管理", value: "serialized" },
                { label: "数量管理", value: "quantity" },
              ]}
              value={values.tracking_mode}
              onValueChange={(value) =>
                updateField("tracking_mode", value as AssetTrackingMode)
              }
            />
          </FormField>
          <FormField htmlFor="asset-editor-default-unit" label="默认单位">
            <Select
              id="asset-editor-default-unit"
              label="默认单位"
              options={units.map((unit) => ({
                label: `${unit.name} (${unit.symbol})`,
                value: unit.unit_id,
              }))}
              value={values.default_unit_id}
              onValueChange={(value) => updateField("default_unit_id", value)}
            />
          </FormField>
        </div>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="asset-model" label="型号">
            <input
              className="input"
              id="asset-model"
              value={values.model}
              onChange={(event) => updateField("model", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="asset-manufacturer-editor" label="厂商">
            <input
              className="input"
              id="asset-manufacturer-editor"
              value={values.manufacturer}
              onChange={(event) => updateField("manufacturer", event.target.value)}
            />
          </FormField>
        </div>
        <FormField htmlFor="asset-public-notes" label="公开备注">
          <textarea
            className="textarea"
            id="asset-public-notes"
            value={values.public_notes}
            onChange={(event) => updateField("public_notes", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="asset-internal-notes" label="内部备注">
          <textarea
            className="textarea"
            id="asset-internal-notes"
            value={values.internal_notes}
            onChange={(event) => updateField("internal_notes", event.target.value)}
          />
        </FormField>
        {isNew ? (
          <PendingAttachmentUploader
            disabled={isSaving}
            laboratoryId={laboratoryId}
            pendingAttachments={pendingAttachments}
            onChange={setPendingAttachments}
          />
        ) : null}
        <label className="checkbox-field" htmlFor="asset-editor-archived">
          <input
            checked={values.is_archived}
            id="asset-editor-archived"
            type="checkbox"
            onChange={(event) => updateField("is_archived", event.target.checked)}
          />
          <span>
            <strong>归档资产</strong>
            <small>归档后仍可查询，但默认业务流程可选择隐藏。</small>
          </span>
        </label>
        {parameters.length > 0 ? (
          <div className="asset-parameter-editor">
            <div>
              <h3 className="asset-editor-section-title">参数值</h3>
              <p className="panel-description">留空会清除已有值；新增资产时留空参数不提交。</p>
            </div>
            {visibleParameters.length > 0 ? (
              visibleParameters.map((parameter) => (
                <ParameterInputField
                  input={parameterInputs[parameter.parameter_type_id] ?? emptyParameterInput()}
                  key={parameter.parameter_type_id}
                  parameter={parameter}
                  required={categoryParameterIds.has(parameter.parameter_type_id)}
                  units={units}
                  unitsById={unitsById}
                  onChange={(next) => updateParameter(parameter.parameter_type_id, next)}
                  onRemove={
                    extraParameterIds.has(parameter.parameter_type_id)
                      ? () => removeExtraParameter(parameter.parameter_type_id)
                      : undefined
                  }
                />
              ))
            ) : (
              <p className="asset-parameter-empty">当前分类没有参数，可添加额外参数。</p>
            )}
            <div className="asset-extra-parameter-row">
              <Select
                disabled={availableExtraParameters.length === 0}
                label="添加额外参数"
                options={[
                  { label: "选择额外参数", value: "none" },
                  ...availableExtraParameters.map((parameter) => ({
                    label: `${parameter.name} (${parameter.code})`,
                    value: parameter.parameter_type_id,
                  })),
                ]}
                value={nextExtraParameterId || "none"}
                onValueChange={(value) => setNextExtraParameterId(value === "none" ? "" : value)}
              />
              <Button
                disabled={!nextExtraParameterId}
                onClick={addExtraParameter}
                variant="primary"
              >
                <Plus size={15} />
                添加额外参数
              </Button>
            </div>
          </div>
        ) : null}
      </form>
    </Dialog>
  );
}

function ParameterInputField({
  input,
  onChange,
  onRemove,
  parameter,
  required,
  units,
  unitsById,
}: {
  input: ParameterInput;
  onChange: (value: Partial<ParameterInput>) => void;
  onRemove?: () => void;
  parameter: AssetParameter;
  required: boolean;
  units: Unit[];
  unitsById: Map<string, Unit>;
}) {
  const unitOptions = units
    .filter((unit) => !parameter.unit_dimension || unit.dimension === parameter.unit_dimension)
    .map((unit) => ({ label: `${unit.name} (${unit.symbol})`, value: unit.unit_id }));
  const defaultUnit = parameter.default_unit_id
    ? unitsById.get(parameter.default_unit_id)
    : null;

  return (
    <div className="asset-parameter-input">
      <div className="asset-parameter-input-label">
        <strong>{parameter.name}</strong>
        <span>
          {parameter.code}
          <Badge tone={required ? "accent" : "default"}>
            {required ? "分类参数" : "额外参数"}
          </Badge>
        </span>
      </div>
      {parameter.data_type === "text" ? (
        <input
          className="input"
          value={input.text}
          onChange={(event) => onChange({ text: event.target.value })}
        />
      ) : null}
      {parameter.data_type === "number" ? (
        <div className="asset-parameter-inline-inputs">
          <input
            className="input"
            type="number"
            value={input.number}
            onChange={(event) => onChange({ number: event.target.value })}
          />
          {unitOptions.length > 0 ? (
            <Select
              label={`${parameter.name} 单位`}
              options={[{ label: "默认单位", value: "default" }, ...unitOptions]}
              value={input.unit_id || "default"}
              onValueChange={(value) => onChange({ unit_id: value === "default" ? "" : value })}
            />
          ) : defaultUnit ? (
            <Badge>{defaultUnit.symbol}</Badge>
          ) : null}
        </div>
      ) : null}
      {parameter.data_type === "range" ? (
        <div className="asset-parameter-inline-inputs">
          <input
            className="input"
            placeholder="起始"
            type="number"
            value={input.range_start}
            onChange={(event) => onChange({ range_start: event.target.value })}
          />
          <input
            className="input"
            placeholder="结束"
            type="number"
            value={input.range_end}
            onChange={(event) => onChange({ range_end: event.target.value })}
          />
          {unitOptions.length > 0 ? (
            <Select
              label={`${parameter.name} 单位`}
              options={[{ label: "默认单位", value: "default" }, ...unitOptions]}
              value={input.unit_id || "default"}
              onValueChange={(value) => onChange({ unit_id: value === "default" ? "" : value })}
            />
          ) : null}
        </div>
      ) : null}
      {parameter.data_type === "boolean" ? (
        <Select
          label={parameter.name}
          options={[
            { label: "未设置", value: "unset" },
            { label: "是", value: "true" },
            { label: "否", value: "false" },
          ]}
          value={input.boolean || "unset"}
          onValueChange={(value) =>
            onChange({ boolean: value === "unset" ? "" : (value as "true" | "false") })
          }
        />
      ) : null}
      {parameter.data_type === "date" ? (
        <input
          className="input"
          type="date"
          value={input.date}
          onChange={(event) => onChange({ date: event.target.value })}
        />
      ) : null}
      {parameter.data_type === "enum" ? (
        <Select
          label={parameter.name}
          options={[
            { label: "未设置", value: "unset" },
            ...parameter.options
              .filter((option) => !option.is_archived)
              .map((option) => ({
                label: option.label,
                value: option.option_id,
              })),
          ]}
          value={input.option_id || "unset"}
          onValueChange={(value) => onChange({ option_id: value === "unset" ? "" : value })}
        />
      ) : null}
      {onRemove ? (
        <Button aria-label="移除额外参数" size="icon" variant="ghost" onClick={onRemove}>
          <Trash2 size={15} />
        </Button>
      ) : null}
    </div>
  );
}

function DeleteAssetDialog({
  asset,
  loading,
  onClose,
  onConfirm,
}: {
  asset: Asset | null;
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open && !loading) onClose();
      }}
      open={asset !== null}
      title="删除资产"
      footer={
        <>
          <Button disabled={loading} onClick={onClose}>
            取消
          </Button>
          <Button disabled={loading} onClick={onConfirm} variant="danger">
            删除
          </Button>
        </>
      }
    >
      <p className="dialog-description">
        {asset
          ? `确认删除“${asset.name}”？该操作会同时删除资产库存项和参数值。`
          : ""}
      </p>
    </Dialog>
  );
}

function buildBasicColumns(
  categoryById: Map<string, AssetCategory>,
  unitsById: Map<string, Unit>,
): AssetColumn[] {
  return [
    {
      key: "asset",
      label: "资产",
      locked: true,
      render: (asset) => <AssetNameCell asset={asset} />,
      sortKey: "name",
    },
    {
      key: "category",
      label: "分类",
      render: (asset) => categoryLabel(asset.category_id, categoryById),
      sortKey: "category",
    },
    {
      key: "tracking_mode",
      label: "模式",
      render: (asset) => <Badge tone="accent">{trackingModeLabel(asset.tracking_mode)}</Badge>,
      sortKey: "tracking_mode",
    },
    {
      key: "manufacturer",
      label: "厂商 / 型号",
      render: (asset) => (
        <span className="asset-muted-cell">
          <strong>{asset.manufacturer ?? "未填写"}</strong>
          <span>{asset.model ?? "未填写型号"}</span>
        </span>
      ),
      sortKey: "manufacturer",
    },
    {
      key: "inventory",
      label: "库存",
      render: (asset) => formatInventory(asset, unitsById),
      sortKey: "inventory",
    },
    {
      key: "archived",
      label: "状态",
      render: (asset) => (
        <Badge tone={asset.is_archived ? "warning" : "success"}>
          {asset.is_archived ? "已归档" : "正常"}
        </Badge>
      ),
      sortKey: "archived",
    },
    {
      key: "updated_at",
      label: "更新时间",
      render: (asset) => formatDate(asset.updated_at),
      sortKey: "updated_at",
    },
  ];
}

function buildParameterColumns(
  parameters: AssetParameter[],
  categoryById: Map<string, AssetCategory>,
  unitsById: Map<string, Unit>,
): AssetColumn[] {
  return [
    {
      key: "asset",
      label: "资产",
      locked: true,
      render: (asset) => <AssetNameCell asset={asset} />,
      sortKey: "name",
    },
    {
      key: "category",
      label: "分类",
      render: (asset) => categoryLabel(asset.category_id, categoryById),
      sortKey: "category",
    },
    ...parameters.map<AssetColumn>((parameter) => ({
      key: parameterColumnKey(parameter),
      label: parameter.name,
      render: (asset) => {
        const value = asset.parameters?.find(
          (candidate) => candidate.parameter_type_id === parameter.parameter_type_id,
        );
        return value ? (
          <span className="asset-parameter-value">
            {formatParameterValue(value, unitsById)}
          </span>
        ) : (
          <span className="muted">未填写</span>
        );
      },
      sortKey: parameterColumnKey(parameter),
    })),
    {
      key: "updated_at",
      label: "更新时间",
      render: (asset) => formatDate(asset.updated_at),
      sortKey: "updated_at",
    },
  ];
}

function AssetNameCell({ asset }: { asset: Asset }) {
  return (
    <span className="asset-name-cell">
      <strong>{asset.name}</strong>
      <span>{asset.model ?? asset.manufacturer ?? asset.asset_id}</span>
    </span>
  );
}

function buildCategoryBreadcrumbs(categoryId: string, categories: AssetCategory[]) {
  const selected = categories.find((category) => category.category_id === categoryId);
  if (!selected) {
    return [{ categoryId: "", label: "全部资产" }];
  }

  const breadcrumbs = [{ categoryId: "", label: "全部资产" }];
  const segments = selected.path.split(".");
  for (let index = 0; index < segments.length; index += 1) {
    const path = segments.slice(0, index + 1).join(".");
    const category = categories.find((candidate) => candidate.path === path);
    if (category) {
      breadcrumbs.push({ categoryId: category.category_id, label: category.name });
    }
  }
  return breadcrumbs;
}

function assetMatchesParameterFilters(
  asset: Asset,
  filters: FilterForm,
  unitsById: Map<string, Unit>,
) {
  const parameterTypeId = filters.parameter_type_id;
  const keyword = filters.parameter_keyword.trim().toLowerCase();
  if (!parameterTypeId && !keyword) {
    return true;
  }

  const values = asset.parameters ?? [];
  const candidates = parameterTypeId
    ? values.filter((value) => value.parameter_type_id === parameterTypeId)
    : values;

  if (parameterTypeId && candidates.length === 0) {
    return false;
  }
  if (!keyword) {
    return true;
  }

  return candidates.some((value) =>
    [
      value.name,
      value.code,
      value.data_type,
      formatParameterValue(value, unitsById),
    ]
      .join(" ")
      .toLowerCase()
      .includes(keyword),
  );
}

function sortAssets(
  assets: Asset[],
  sort: SortState,
  categoryById: Map<string, AssetCategory>,
  unitsById: Map<string, Unit>,
) {
  return [...assets].sort((left, right) => {
    const leftValue = sortValue(left, sort.key, categoryById, unitsById);
    const rightValue = sortValue(right, sort.key, categoryById, unitsById);
    const result =
      typeof leftValue === "number" && typeof rightValue === "number"
        ? leftValue - rightValue
        : String(leftValue).localeCompare(String(rightValue), "zh-CN", {
            numeric: true,
          });
    return sort.direction === "asc" ? result : -result;
  });
}

function sortValue(
  asset: Asset,
  key: string,
  categoryById: Map<string, AssetCategory>,
  unitsById: Map<string, Unit>,
) {
  if (key.startsWith("param:")) {
    const parameterId = key.slice("param:".length);
    const value = asset.parameters?.find(
      (candidate) => candidate.parameter_type_id === parameterId,
    );
    return value ? formatParameterValue(value, unitsById) : "";
  }

  switch (key) {
    case "archived":
      return asset.is_archived ? 1 : 0;
    case "category":
      return categoryLabel(asset.category_id, categoryById);
    case "inventory":
      return asset.inventory_summary.quantity_on_hand;
    case "manufacturer":
      return `${asset.manufacturer ?? ""} ${asset.model ?? ""}`;
    case "tracking_mode":
      return asset.tracking_mode;
    case "updated_at":
      return new Date(asset.updated_at).getTime();
    case "name":
    default:
      return asset.name;
  }
}

function formatInventory(asset: Asset, unitsById: Map<string, Unit>) {
  const unit = unitsById.get(asset.default_unit_id);
  const quantity = formatNumber(asset.inventory_summary.quantity_on_hand);
  const allocated = formatNumber(asset.inventory_summary.quantity_allocated);
  const unitSymbol = unit ? ` ${unit.symbol}` : "";
  return (
    <span className="asset-muted-cell">
      <strong>
        {asset.inventory_summary.item_count} 项 · {quantity}
        {unitSymbol}
      </strong>
      <span>已分配 {allocated}{unitSymbol}</span>
    </span>
  );
}

function formatParameterValue(value: AssetParameterValue, unitsById: Map<string, Unit>) {
  const runtimeValue = value.value;
  if (value.data_type === "text") {
    return runtimeValue.text ?? "";
  }
  if (value.data_type === "number") {
    const unit = runtimeValue.unit_id ? unitsById.get(runtimeValue.unit_id) : null;
    return `${formatNumber(runtimeValue.number ?? 0)}${unit ? ` ${unit.symbol}` : ""}`;
  }
  if (value.data_type === "range") {
    const unit = runtimeValue.unit_id ? unitsById.get(runtimeValue.unit_id) : null;
    return `${formatNumber(runtimeValue.range_start ?? 0)} - ${formatNumber(
      runtimeValue.range_end ?? 0,
    )}${unit ? ` ${unit.symbol}` : ""}`;
  }
  if (value.data_type === "boolean") {
    return runtimeValue.boolean ? "是" : "否";
  }
  if (value.data_type === "date") {
    return runtimeValue.date ?? "";
  }
  if (value.data_type === "enum") {
    return runtimeValue.option_label ?? runtimeValue.option_code ?? runtimeValue.option_id ?? "";
  }
  return "";
}

function parameterIdsForCategory(categoryId: string, categories: AssetCategory[]) {
  const selectedCategory = categories.find((category) => category.category_id === categoryId);
  const ids = new Set<string>();
  if (!selectedCategory) {
    return ids;
  }

  const ancestors = categories
    .filter(
      (category) =>
        selectedCategory.path === category.path ||
        selectedCategory.path.startsWith(`${category.path}.`),
    )
    .sort((left, right) => left.depth - right.depth);

  for (const category of ancestors) {
    for (const assignment of category.parameter_assignments) {
      if (
        category.category_id === selectedCategory.category_id ||
        assignment.applies_to_descendants
      ) {
        ids.add(assignment.parameter_type_id);
      }
    }
  }
  return ids;
}

function parametersForEditor({
  categoryParameterIds,
  extraParameterIds,
  parameterById,
  parameters,
}: {
  categoryParameterIds: Set<string>;
  extraParameterIds: Set<string>;
  parameterById: Map<string, AssetParameter>;
  parameters: AssetParameter[];
}) {
  const ordered: AssetParameter[] = [];
  const seen = new Set<string>();

  for (const parameter of parameters) {
    if (categoryParameterIds.has(parameter.parameter_type_id)) {
      ordered.push(parameter);
      seen.add(parameter.parameter_type_id);
    }
  }

  for (const parameterId of extraParameterIds) {
    if (seen.has(parameterId)) {
      continue;
    }
    const parameter = parameterById.get(parameterId);
    if (parameter) {
      ordered.push(parameter);
      seen.add(parameterId);
    }
  }

  return ordered;
}

function extraParameterIdsFromValues(
  asset: Asset,
  categories: AssetCategory[],
  parameters: AssetParameter[],
) {
  const categoryParameterIds = parameterIdsForCategory(asset.category_id ?? "", categories);
  const knownParameterIds = new Set(parameters.map((parameter) => parameter.parameter_type_id));
  return new Set(
    (asset.parameters ?? [])
      .map((value) => value.parameter_type_id)
      .filter(
        (parameterId) =>
          knownParameterIds.has(parameterId) && !categoryParameterIds.has(parameterId),
      ),
  );
}

function serializeParameterPayloads({
  existingValues,
  inputs,
  parameters,
  removedParameterIds,
}: {
  existingValues: AssetParameterValue[];
  inputs: Record<string, ParameterInput>;
  parameters: AssetParameter[];
  removedParameterIds: Set<string>;
}): ParameterPayloadResult {
  const existingIds = new Set(existingValues.map((value) => value.parameter_type_id));
  const values: AssetParameterValuePayload[] = [];

  for (const parameterId of removedParameterIds) {
    values.push({
      parameter_type_id: parameterId,
      value: null,
    });
  }

  for (const parameter of parameters) {
    if (removedParameterIds.has(parameter.parameter_type_id)) {
      continue;
    }
    const input = inputs[parameter.parameter_type_id] ?? emptyParameterInput();
    const serialized = serializeParameterValue(parameter, input);
    if (!serialized.ok) {
      return serialized;
    }
    if (serialized.value === undefined) {
      continue;
    }
    if (serialized.value === null && !existingIds.has(parameter.parameter_type_id)) {
      continue;
    }
    values.push({
      parameter_type_id: parameter.parameter_type_id,
      value: serialized.value,
    });
  }

  return { ok: true, values };
}

function serializeParameterValue(
  parameter: AssetParameter,
  input: ParameterInput,
):
  | { ok: true; value: AssetParameterPayloadValue | null | undefined }
  | { message: string; ok: false } {
  if (parameter.data_type === "text") {
    const text = input.text.trim();
    return { ok: true, value: text ? { text } : null };
  }
  if (parameter.data_type === "number") {
    if (!input.number.trim()) return { ok: true, value: null };
    const number = Number(input.number);
    if (!Number.isFinite(number)) {
      return { ok: false, message: `${parameter.name} 必须是有效数字` };
    }
    return {
      ok: true,
      value: { number, unit_id: input.unit_id || undefined },
    };
  }
  if (parameter.data_type === "range") {
    if (!input.range_start.trim() && !input.range_end.trim()) {
      return { ok: true, value: null };
    }
    const rangeStart = Number(input.range_start);
    const rangeEnd = Number(input.range_end);
    if (!Number.isFinite(rangeStart) || !Number.isFinite(rangeEnd)) {
      return { ok: false, message: `${parameter.name} 必须填写有效范围` };
    }
    return {
      ok: true,
      value: {
        range_end: rangeEnd,
        range_start: rangeStart,
        unit_id: input.unit_id || undefined,
      },
    };
  }
  if (parameter.data_type === "boolean") {
    if (!input.boolean) return { ok: true, value: null };
    return { ok: true, value: { boolean: input.boolean === "true" } };
  }
  if (parameter.data_type === "date") {
    return { ok: true, value: input.date ? { date: input.date } : null };
  }
  if (parameter.data_type === "enum") {
    return { ok: true, value: input.option_id ? { option_id: input.option_id } : null };
  }
  return { ok: true, value: undefined };
}

function buildParameterInputs(
  parameters: AssetParameter[],
  units: Unit[],
  values: AssetParameterValue[] | null,
) {
  const valuesByParameterId = new Map(
    (values ?? []).map((value) => [value.parameter_type_id, value]),
  );
  const result: Record<string, ParameterInput> = {};
  for (const parameter of parameters) {
    const value = valuesByParameterId.get(parameter.parameter_type_id);
    result[parameter.parameter_type_id] = value
      ? inputFromParameterValue(value)
      : emptyParameterInput(defaultUnitForParameter(parameter, units));
  }
  return result;
}

function inputFromParameterValue(value: AssetParameterValue): ParameterInput {
  const runtimeValue = value.value;
  return {
    boolean:
      runtimeValue.boolean === undefined || runtimeValue.boolean === null
        ? ""
        : runtimeValue.boolean
          ? "true"
          : "false",
    date: runtimeValue.date ?? "",
    number: runtimeValue.number === undefined || runtimeValue.number === null ? "" : String(runtimeValue.number),
    option_id: runtimeValue.option_id ?? "",
    range_end:
      runtimeValue.range_end === undefined || runtimeValue.range_end === null
        ? ""
        : String(runtimeValue.range_end),
    range_start:
      runtimeValue.range_start === undefined || runtimeValue.range_start === null
        ? ""
        : String(runtimeValue.range_start),
    text: runtimeValue.text ?? "",
    unit_id: runtimeValue.unit_id ?? "",
  };
}

function emptyAssetForm(units: Unit[] = []): AssetForm {
  return {
    category_id: "",
    default_unit_id: units[0]?.unit_id ?? "",
    internal_notes: "",
    is_archived: false,
    manufacturer: "",
    model: "",
    name: "",
    public_notes: "",
    tracking_mode: "quantity",
  };
}

function emptyParameterInput(unitId = ""): ParameterInput {
  return {
    boolean: "",
    date: "",
    number: "",
    option_id: "",
    range_end: "",
    range_start: "",
    text: "",
    unit_id: unitId,
  };
}

function defaultUnitForParameter(parameter: AssetParameter, units: Unit[]) {
  if (parameter.default_unit_id) {
    return parameter.default_unit_id;
  }
  return units.find((unit) => unit.dimension === parameter.unit_dimension)?.unit_id ?? "";
}

function emptyFilters(categoryId = "", exactCategory = false, locationId = ""): FilterForm {
  return {
    category_id: categoryId,
    exact_category: exactCategory,
    has_inventory: "all",
    inventory_status: "",
    is_archived: "false",
    keyword: "",
    location_id: locationId,
    manufacturer: "",
    parameter_keyword: "",
    parameter_type_id: "",
    tracking_mode: "all",
  };
}

function optional(value: string) {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function optionalBoolean(value: "all" | "true" | "false") {
  if (value === "true") return true;
  if (value === "false") return false;
  return undefined;
}

function mapById<T extends Record<K, string>, K extends keyof T>(items: T[], key: K) {
  return new Map(items.map((item) => [item[key], item]));
}

function categoryLabel(categoryId: string | null, categoryById: Map<string, AssetCategory>) {
  if (!categoryId) {
    return "未分类";
  }
  const category = categoryById.get(categoryId);
  return category ? category.path.replaceAll(".", " / ") : "未知分类";
}

function parameterColumnKey(parameter: AssetParameter) {
  return `param:${parameter.parameter_type_id}`;
}

function trackingModeLabel(mode: AssetTrackingMode) {
  return mode === "serialized" ? "序列号" : "数量";
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: 4,
  }).format(value);
}
