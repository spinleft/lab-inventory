import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useQueries } from "@tanstack/react-query";
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
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";
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
  type AssetParameterValue,
  type AssetQuery,
  type AssetTrackingMode,
  assetQueryKeys,
  assetSchema,
  useAssets,
} from "../assets/api";
import {
  type CreateInventoryItemsPayload,
  type InventoryItem,
  type InventoryItemQuery,
  type InventoryStatus,
  type UpdateInventoryItemPayload,
  useCreateInventoryItems,
  useDeleteInventoryItem,
  useInventoryItems,
  useUpdateInventoryItem,
} from "./api";
import {
  categoryLabel,
  formatNumber,
  formatParameterValue,
  inventoryStatusLabel,
  inventoryStatusTone,
  locationLabel,
  parameterColumnKey,
  trackingModeLabel,
  unitLabel,
} from "./format";

type ViewMode = "basic" | "parameters";

type FilterForm = {
  batch_number: string;
  category_id: string;
  exact_category: boolean;
  has_batch: "all" | "true" | "false";
  has_location: "all" | "true" | "false";
  keyword: string;
  location_id: string;
  parameter_keyword: string;
  parameter_type_id: string;
  serial_number: string;
  status: "all" | InventoryStatus;
  tracking_mode: "all" | AssetTrackingMode;
};

type SortState = {
  direction: "asc" | "desc";
  key: string;
};

type InventoryColumn = {
  align?: "left" | "right";
  key: string;
  label: string;
  locked?: boolean;
  render: (item: InventoryItem) => ReactNode;
  sortKey?: string;
};

type InventoryEditorState =
  | { mode: "create" }
  | { item: InventoryItem; mode: "edit" }
  | null;

type InventoryForm = {
  asset_id: string;
  batch_number: string;
  count: string;
  internal_notes: string;
  location_id: string;
  public_notes: string;
  quantity_allocated: string;
  quantity_on_hand: string;
  quantity_unit_id: string;
  serial_mode: "serials" | "count";
  serial_number: string;
  serial_numbers: string;
  status: InventoryStatus;
};

const PAGE_SIZE = 30;
const EMPTY_CATEGORIES: AssetCategory[] = [];
const EMPTY_PARAMETERS: AssetParameter[] = [];
const EMPTY_LOCATIONS: Location[] = [];
const EMPTY_UNITS: Unit[] = [];
const DEFAULT_BASIC_COLUMNS = [
  "asset",
  "item",
  "category",
  "status",
  "location",
  "batch",
  "updated_at",
];
const DEFAULT_PARAMETER_BASE_COLUMNS = ["asset", "item", "status", "location"];

export function InventoryPage() {
  const {
    canManageSelectedLaboratoryAssets,
    selectedLaboratoryId,
    selectedLaboratoryName,
  } = useLaboratorySelection();
  const { apiBaseUrl } = useBackendConfig();
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
  const [sort, setSort] = useState<SortState>({ direction: "asc", key: "updated_at" });
  const [visibleBasicColumns, setVisibleBasicColumns] = useState<Set<string>>(
    () => new Set(DEFAULT_BASIC_COLUMNS),
  );
  const [visibleParameterColumns, setVisibleParameterColumns] = useState<Set<string>>(
    () => new Set(DEFAULT_PARAMETER_BASE_COLUMNS),
  );
  const [editor, setEditor] = useState<InventoryEditorState>(null);
  const [deletingItem, setDeletingItem] = useState<InventoryItem | null>(null);

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
  const createInventoryItems = useCreateInventoryItems();
  const updateInventoryItem = useUpdateInventoryItem();
  const deleteInventoryItem = useDeleteInventoryItem();

  const categories = categoriesQuery.data ?? EMPTY_CATEGORIES;
  const parameters = parametersQuery.data ?? EMPTY_PARAMETERS;
  const locations = locationsQuery.data ?? EMPTY_LOCATIONS;
  const units = unitsQuery.data ?? EMPTY_UNITS;
  const categoryById = useMemo(() => mapById(categories, "category_id"), [categories]);
  const locationById = useMemo(() => mapById(locations, "location_id"), [locations]);
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
    const parameterKeys = parameters.map((parameter) =>
      parameterColumnKey(parameter.parameter_type_id),
    );
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

  const query = useMemo<InventoryItemQuery>(
    () => ({
      batch_number: optional(filters.batch_number),
      category_id: optional(filters.category_id),
      exact_category: filters.exact_category || undefined,
      has_batch: optionalBoolean(filters.has_batch),
      has_location: optionalBoolean(filters.has_location),
      keyword: optional(filters.keyword),
      limit: PAGE_SIZE,
      location_id: optional(filters.location_id),
      offset,
      serial_number: optional(filters.serial_number),
      status: filters.status === "all" ? undefined : filters.status,
      tracking_mode:
        filters.tracking_mode === "all" ? undefined : filters.tracking_mode,
    }),
    [filters, offset],
  );
  const inventoryQuery = useInventoryItems({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    query,
  });
  const response = inventoryQuery.data;
  const total = response?.total ?? 0;
  const page = Math.floor(offset / PAGE_SIZE) + 1;
  const maxPage = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const parameterFilterActive = Boolean(
    filters.parameter_type_id || filters.parameter_keyword.trim(),
  );
  const needsAssetDetails = viewMode === "parameters" || parameterFilterActive;
  const currentPageAssetIds = useMemo(
    () => Array.from(new Set((response?.items ?? []).map((item) => item.asset_id))),
    [response?.items],
  );
  const assetDetailQueries = useQueries({
    queries: currentPageAssetIds.map((assetId) => ({
      enabled: Boolean(apiBaseUrl) && needsAssetDetails,
      queryKey: assetQueryKeys.detail(apiBaseUrl, assetId, true),
      queryFn: async () => {
        const client = createApiClient(apiBaseUrl);
        return assetSchema.parse(
          await client.get(`/assets/${assetId}`, { include: "parameters" }),
        );
      },
    })),
  });
  const assetDetailsById = useMemo(() => {
    const next = new Map<string, Asset>();
    for (const queryResult of assetDetailQueries) {
      if (queryResult.data) {
        next.set(queryResult.data.asset_id, queryResult.data);
      }
    }
    return next;
  }, [assetDetailQueries]);
  const assetDetailsLoading =
    needsAssetDetails &&
    assetDetailQueries.some((queryResult) => queryResult.isLoading || queryResult.isFetching);

  const parameterFilteredItems = useMemo(
    () =>
      (response?.items ?? []).filter((item) =>
        itemMatchesParameterFilters(item, filters, assetDetailsById, unitsById),
      ),
    [assetDetailsById, filters, response?.items, unitsById],
  );
  const visibleItems = useMemo(
    () =>
      sortInventoryItems(
        parameterFilteredItems,
        sort,
        categoryById,
        locationById,
        unitsById,
        assetDetailsById,
      ),
    [assetDetailsById, categoryById, locationById, parameterFilteredItems, sort, unitsById],
  );
  const locationBreadcrumbs = useMemo(
    () => buildLocationBreadcrumbs(filters.location_id, locations),
    [filters.location_id, locations],
  );
  const basicColumns = useMemo(
    () => buildBasicColumns(categoryById, locationById, unitsById),
    [categoryById, locationById, unitsById],
  );
  const parameterColumns = useMemo(
    () =>
      buildParameterColumns(
        parameters,
        categoryById,
        locationById,
        unitsById,
        assetDetailsById,
      ),
    [assetDetailsById, categoryById, locationById, parameters, unitsById],
  );
  const activeColumns = viewMode === "basic" ? basicColumns : parameterColumns;
  const visibleColumnKeys =
    viewMode === "basic" ? visibleBasicColumns : visibleParameterColumns;
  const visibleColumns = activeColumns.filter(
    (column) => column.locked || visibleColumnKeys.has(column.key),
  );

  function submitFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setOffset(0);
    setFilters(draftFilters);
    syncInventorySearch(
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

  function syncInventorySearch(categoryId: string, exactCategory: boolean, locationId: string) {
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
    if (!deletingItem) return;
    deleteInventoryItem.mutate(deletingItem.inventory_item_id, {
      onError: (error) =>
        toast.error({ title: "删除库存失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        toast.success({ title: "库存已删除" });
        setDeletingItem(null);
      },
    });
  }

  const pageActions = (
    <Button
      disabled={!canManage || !selectedLaboratoryId}
      onClick={() => setEditor({ mode: "create" })}
      variant="primary"
    >
      <Plus size={15} />
      添加库存
    </Button>
  );

  return (
    <main className="page">
      <PageHeader
        kicker="库存"
        title="库存"
        description="查看资产库存项，并按分类、位置、批号、状态和参数缩小查询范围。"
        actions={pageActions}
      />

      <LocationPathNav breadcrumbs={locationBreadcrumbs} />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">查询条件</h2>
            <p className="panel-description">
              {selectedLaboratoryName || "请选择实验室"} · 参数筛选作用于当前页库存关联资产
            </p>
          </div>
        </div>
        <div className="panel-body">
          <form className="asset-filter-form" onSubmit={submitFilters}>
            <div className="asset-filter-grid">
              <FormField htmlFor="inventory-keyword" label="关键词">
                <input
                  className="input"
                  id="inventory-keyword"
                  placeholder="资产、型号、厂商、序列号、批号或备注"
                  value={draftFilters.keyword}
                  onChange={(event) => updateDraftFilter("keyword", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="inventory-category" label="资产分类">
                <Select
                  id="inventory-category"
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
              <FormField htmlFor="inventory-tracking-mode" label="管理模式">
                <Select
                  id="inventory-tracking-mode"
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
              <FormField htmlFor="inventory-status" label="库存状态">
                <Select
                  id="inventory-status"
                  label="库存状态"
                  options={[
                    { label: "全部状态", value: "all" },
                    { label: "可用", value: "available" },
                    { label: "预留", value: "reserved" },
                    { label: "退役", value: "retired" },
                    { label: "丢失", value: "lost" },
                    { label: "已消耗", value: "consumed" },
                  ]}
                  value={draftFilters.status}
                  onValueChange={(value) =>
                    updateDraftFilter("status", value as FilterForm["status"])
                  }
                />
              </FormField>
              <FormField htmlFor="inventory-location" label="位置">
                <Select
                  id="inventory-location"
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
              <FormField htmlFor="inventory-serial" label="序列号">
                <input
                  className="input"
                  id="inventory-serial"
                  value={draftFilters.serial_number}
                  onChange={(event) => updateDraftFilter("serial_number", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="inventory-batch" label="批号">
                <input
                  className="input"
                  id="inventory-batch"
                  value={draftFilters.batch_number}
                  onChange={(event) => updateDraftFilter("batch_number", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="inventory-has-batch" label="是否有批号">
                <Select
                  id="inventory-has-batch"
                  label="是否有批号"
                  options={[
                    { label: "全部", value: "all" },
                    { label: "有批号", value: "true" },
                    { label: "无批号", value: "false" },
                  ]}
                  value={draftFilters.has_batch}
                  onValueChange={(value) =>
                    updateDraftFilter("has_batch", value as FilterForm["has_batch"])
                  }
                />
              </FormField>
              <FormField htmlFor="inventory-has-location" label="是否有位置">
                <Select
                  id="inventory-has-location"
                  label="是否有位置"
                  options={[
                    { label: "全部", value: "all" },
                    { label: "有位置", value: "true" },
                    { label: "无位置", value: "false" },
                  ]}
                  value={draftFilters.has_location}
                  onValueChange={(value) =>
                    updateDraftFilter("has_location", value as FilterForm["has_location"])
                  }
                />
              </FormField>
              <FormField htmlFor="inventory-parameter-type" label="参数类型">
                <Select
                  id="inventory-parameter-type"
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
              <FormField htmlFor="inventory-parameter-keyword" label="参数搜索">
                <input
                  className="input"
                  id="inventory-parameter-keyword"
                  placeholder="参数名、代码或值"
                  value={draftFilters.parameter_keyword}
                  onChange={(event) =>
                    updateDraftFilter("parameter_keyword", event.target.value)
                  }
                />
              </FormField>
            </div>
            <label className="checkbox-field asset-filter-checkbox" htmlFor="inventory-exact-category">
              <input
                checked={draftFilters.exact_category}
                disabled={!draftFilters.category_id}
                id="inventory-exact-category"
                type="checkbox"
                onChange={(event) =>
                  updateDraftFilter("exact_category", event.target.checked)
                }
              />
              <span>
                <strong>精确分类</strong>
                <small>关闭时包含所选分类的所有子分类库存。</small>
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
            <h2 className="panel-title">库存列表</h2>
            <p className="panel-description">
              第 {page} / {maxPage} 页，共 {total} 条
              {parameterFilterActive ? `，当前页参数匹配 ${visibleItems.length} 条` : ""}
            </p>
          </div>
          <div className="toolbar-group">
            <div className="tabs-list asset-view-tabs" role="tablist" aria-label="库存视图">
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
              disabled={offset <= 0 || inventoryQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="上一页"
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              <ChevronLeft size={16} />
            </Button>
            <Button
              disabled={offset + PAGE_SIZE >= total || inventoryQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="下一页"
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              <ChevronRight size={16} />
            </Button>
          </div>
        </div>
        <InventoryTable
          canManage={canManage}
          columns={visibleColumns}
          items={visibleItems}
          loading={
            inventoryQuery.isLoading ||
            categoriesQuery.isLoading ||
            locationsQuery.isLoading ||
            parametersQuery.isLoading ||
            unitsQuery.isLoading ||
            assetDetailsLoading
          }
          sort={sort}
          onDelete={setDeletingItem}
          onEdit={(item) => setEditor({ item, mode: "edit" })}
          onRowClick={(item) => navigate(`/inventory/${item.inventory_item_id}`)}
          onSort={handleSort}
        />
      </section>

      <InventoryEditor
        categories={categories}
        createInventoryItems={createInventoryItems}
        editor={editor}
        laboratoryId={selectedLaboratoryId}
        locations={locations}
        open={editor !== null}
        units={units}
        updateInventoryItem={updateInventoryItem}
        onClose={() => setEditor(null)}
        onSaved={() => setEditor(null)}
      />
      <DeleteInventoryDialog
        item={deletingItem}
        loading={deleteInventoryItem.isPending}
        onClose={() => setDeletingItem(null)}
        onConfirm={confirmDelete}
      />
    </main>
  );
}

function LocationPathNav({
  breadcrumbs,
}: {
  breadcrumbs: Array<{ label: string; locationId: string }>;
}) {
  return (
    <nav className="asset-category-path" aria-label="库存位置路径">
      {breadcrumbs.map((breadcrumb, index) => (
        <span className="asset-category-path-item" key={breadcrumb.locationId || "all"}>
          {index > 0 ? <span className="asset-category-path-separator">/</span> : null}
          <Link to={breadcrumb.locationId ? `/assets?location_id=${breadcrumb.locationId}` : "/assets"}>
            {breadcrumb.label}
          </Link>
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
  columns: InventoryColumn[];
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

function InventoryTable({
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
  columns: InventoryColumn[];
  items: InventoryItem[];
  loading: boolean;
  onDelete: (item: InventoryItem) => void;
  onEdit: (item: InventoryItem) => void;
  onRowClick: (item: InventoryItem) => void;
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
        description="当前查询条件下没有库存项。"
        title="暂无库存"
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
          {items.map((item) => (
            <tr
              className="asset-clickable-row"
              key={item.inventory_item_id}
              tabIndex={0}
              onClick={() => onRowClick(item)}
              onKeyDown={(event) => {
                if (event.key === "Enter") onRowClick(item);
              }}
            >
              {columns.map((column) => (
                <td
                  key={column.key}
                  style={{ textAlign: column.align === "right" ? "right" : "left" }}
                >
                  {column.render(item)}
                </td>
              ))}
              <td style={{ textAlign: "right" }} onClick={(event) => event.stopPropagation()}>
                <InventoryActions
                  canManage={canManage}
                  item={item}
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

function InventoryActions({
  canManage,
  item,
  onDelete,
  onEdit,
}: {
  canManage: boolean;
  item: InventoryItem;
  onDelete: (item: InventoryItem) => void;
  onEdit: (item: InventoryItem) => void;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          aria-label={`库存 ${inventoryItemLabel(item)} 操作`}
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
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onEdit(item)}>
            <Pencil size={15} />
            编辑库存
          </DropdownMenu.Item>
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onDelete(item)}>
            <Trash2 size={15} />
            删除库存
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function InventoryEditor({
  categories,
  createInventoryItems,
  editor,
  laboratoryId,
  locations,
  onClose,
  onSaved,
  open,
  units,
  updateInventoryItem,
}: {
  categories: AssetCategory[];
  createInventoryItems: ReturnType<typeof useCreateInventoryItems>;
  editor: InventoryEditorState;
  laboratoryId: string;
  locations: Location[];
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  units: Unit[];
  updateInventoryItem: ReturnType<typeof useUpdateInventoryItem>;
}) {
  const toast = useToast();
  const isCreate = editor?.mode === "create";
  const editingItem = editor?.mode === "edit" ? editor.item : null;
  const [assetSearch, setAssetSearch] = useState("");
  const [values, setValues] = useState<InventoryForm>(() => emptyInventoryForm());
  const [pendingAttachments, setPendingAttachments] = useState<PendingAttachment[]>([]);
  const unitsById = useMemo(() => mapById(units, "unit_id"), [units]);
  const assetQuery = useMemo<AssetQuery>(
    () => ({
      is_archived: false,
      keyword: optional(assetSearch),
      limit: 50,
      offset: 0,
    }),
    [assetSearch],
  );
  const assetsQuery = useAssets({
    enabled: open && isCreate && Boolean(laboratoryId),
    laboratoryId,
    query: assetQuery,
  });
  const assetOptions = assetsQuery.data?.items ?? [];
  const selectedAsset = isCreate
    ? assetOptions.find((asset) => asset.asset_id === values.asset_id)
    : null;
  const trackingMode = editingItem?.tracking_mode ?? selectedAsset?.tracking_mode ?? null;
  const quantityUnitId =
    editingItem?.asset.default_unit_id ?? selectedAsset?.default_unit_id ?? values.quantity_unit_id;
  const isSaving = createInventoryItems.isPending || updateInventoryItem.isPending;

  useEffect(() => {
    if (!editor) {
      setAssetSearch("");
      setValues(emptyInventoryForm());
      setPendingAttachments([]);
      return;
    }

    if (editor.mode === "create") {
      setValues(emptyInventoryForm());
      setPendingAttachments([]);
      return;
    }

    setValues(formFromInventoryItem(editor.item));
    setPendingAttachments([]);
  }, [editor]);

  useEffect(() => {
    if (!isCreate || !selectedAsset) {
      return;
    }
    setValues((current) => {
      return { ...current, quantity_unit_id: selectedAsset.default_unit_id };
    });
  }, [isCreate, selectedAsset]);

  function updateField<K extends keyof InventoryForm>(key: K, value: InventoryForm[K]) {
    setValues((current) => ({ ...current, [key]: value }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (isCreate) {
      if (!selectedAsset) {
        toast.error({ title: "请选择资产" });
        return;
      }
      const payloadResult = buildCreatePayload(values, selectedAsset);
      if (!payloadResult.ok) {
        toast.error({ title: payloadResult.message });
        return;
      }
      const attachmentClaims = attachmentClaimsFromPending(pendingAttachments);
      if (!attachmentClaims.ok) {
        toast.error({ title: attachmentClaims.message });
        return;
      }
      if (
        attachmentClaims.claims.length > 0 &&
        !canAttachToCreatedInventoryItem(values, selectedAsset)
      ) {
        toast.error({ title: "附件只能在本次创建单个库存项时添加。" });
        return;
      }
      createInventoryItems.mutate(
        {
          assetId: selectedAsset.asset_id,
          payload: {
            ...payloadResult.payload,
            attachments:
              attachmentClaims.claims.length > 0 ? attachmentClaims.claims : undefined,
          },
        },
        {
          onError: (error) =>
            toast.error({ title: "创建库存失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            setPendingAttachments([]);
            toast.success({ title: "库存已创建" });
            onSaved();
          },
        },
      );
      return;
    }

    if (editingItem) {
      const payloadResult = buildUpdatePayload(values, editingItem);
      if (!payloadResult.ok) {
        toast.error({ title: payloadResult.message });
        return;
      }
      updateInventoryItem.mutate(
        {
          inventoryItemId: editingItem.inventory_item_id,
          payload: payloadResult.payload,
        },
        {
          onError: (error) =>
            toast.error({ title: "更新库存失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "库存已更新" });
            onSaved();
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="根据资产管理模式创建或调整库存项。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isCreate ? "添加库存" : "编辑库存"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="inventory-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="inventory-form" onSubmit={handleSubmit}>
        {isCreate ? (
          <>
            <FormField htmlFor="inventory-asset-search" label="搜索资产">
              <input
                className="input"
                id="inventory-asset-search"
                placeholder="资产名称、型号或厂商"
                value={assetSearch}
                onChange={(event) => setAssetSearch(event.target.value)}
              />
            </FormField>
            <FormField htmlFor="inventory-editor-asset" label="资产">
              <Select
                disabled={assetsQuery.isLoading || assetOptions.length === 0}
                id="inventory-editor-asset"
                label="资产"
                options={[
                  { label: "选择资产", value: "none" },
                  ...assetOptions.map((asset) => ({
                    label: `${asset.name}${asset.model ? ` · ${asset.model}` : ""}`,
                    value: asset.asset_id,
                  })),
                ]}
                value={values.asset_id || "none"}
                onValueChange={(value) =>
                  updateField("asset_id", value === "none" ? "" : value)
                }
              />
            </FormField>
            {selectedAsset ? (
              <div className="asset-detail-item">
                <dt>所选资产</dt>
                <dd>
                  {selectedAsset.name} · {trackingModeLabel(selectedAsset.tracking_mode)}
                </dd>
              </div>
            ) : null}
          </>
        ) : editingItem ? (
          <div className="asset-detail-item">
            <dt>库存项</dt>
            <dd>
              {editingItem.asset.name} · {inventoryItemLabel(editingItem)}
            </dd>
          </div>
        ) : null}

        {trackingMode === "serialized" ? (
          <SerializedInventoryFields
            editing={Boolean(editingItem)}
            values={values}
            onChange={updateField}
          />
        ) : null}

        {trackingMode === "quantity" ? (
          <QuantityInventoryFields
            unitId={quantityUnitId}
            unitsById={unitsById}
            values={values}
            onChange={updateField}
          />
        ) : null}

        {trackingMode ? (
          <>
            <div className="form-grid form-grid-2">
              <FormField htmlFor="inventory-editor-status" label="状态">
                <Select
                  id="inventory-editor-status"
                  label="状态"
                  options={inventoryStatusOptions()}
                  value={values.status}
                  onValueChange={(value) => updateField("status", value as InventoryStatus)}
                />
              </FormField>
              <FormField htmlFor="inventory-editor-location" label="位置">
                <Select
                  id="inventory-editor-location"
                  label="位置"
                  options={[
                    { label: "未设置", value: "none" },
                    ...locations.map((location) => ({
                      label: `${"　".repeat(location.depth)}${location.name}`,
                      value: location.location_id,
                    })),
                  ]}
                  value={values.location_id || "none"}
                  onValueChange={(value) =>
                    updateField("location_id", value === "none" ? "" : value)
                  }
                />
              </FormField>
            </div>
            <FormField htmlFor="inventory-editor-batch" label="批号">
              <input
                className="input"
                id="inventory-editor-batch"
                value={values.batch_number}
                onChange={(event) => updateField("batch_number", event.target.value)}
              />
            </FormField>
            <FormField htmlFor="inventory-editor-public-notes" label="公开备注">
              <textarea
                className="textarea"
                id="inventory-editor-public-notes"
                value={values.public_notes}
                onChange={(event) => updateField("public_notes", event.target.value)}
              />
            </FormField>
            <FormField htmlFor="inventory-editor-internal-notes" label="内部备注">
              <textarea
                className="textarea"
                id="inventory-editor-internal-notes"
                value={values.internal_notes}
                onChange={(event) => updateField("internal_notes", event.target.value)}
              />
            </FormField>
            {isCreate ? (
              <PendingAttachmentUploader
                disabled={isSaving}
                laboratoryId={laboratoryId}
                pendingAttachments={pendingAttachments}
                onChange={setPendingAttachments}
              />
            ) : null}
          </>
        ) : null}
      </form>
    </Dialog>
  );
}

function SerializedInventoryFields({
  editing,
  onChange,
  values,
}: {
  editing: boolean;
  onChange: <K extends keyof InventoryForm>(key: K, value: InventoryForm[K]) => void;
  values: InventoryForm;
}) {
  if (editing) {
    return (
      <FormField htmlFor="inventory-editor-serial-number" label="序列号">
        <input
          className="input"
          id="inventory-editor-serial-number"
          value={values.serial_number}
          onChange={(event) => onChange("serial_number", event.target.value)}
        />
      </FormField>
    );
  }

  return (
    <>
      <FormField htmlFor="inventory-editor-serial-mode" label="创建方式">
        <Select
          id="inventory-editor-serial-mode"
          label="创建方式"
          options={[
            { label: "输入序列号", value: "serials" },
            { label: "自动生成 #N", value: "count" },
          ]}
          value={values.serial_mode}
          onValueChange={(value) => onChange("serial_mode", value as InventoryForm["serial_mode"])}
        />
      </FormField>
      {values.serial_mode === "serials" ? (
        <FormField htmlFor="inventory-editor-serial-numbers" label="序列号">
          <textarea
            className="textarea"
            id="inventory-editor-serial-numbers"
            placeholder="每行或用逗号分隔一个序列号"
            value={values.serial_numbers}
            onChange={(event) => onChange("serial_numbers", event.target.value)}
          />
        </FormField>
      ) : (
        <FormField htmlFor="inventory-editor-count" label="创建数量">
          <input
            className="input"
            id="inventory-editor-count"
            min={1}
            type="number"
            value={values.count}
            onChange={(event) => onChange("count", event.target.value)}
          />
        </FormField>
      )}
    </>
  );
}

function QuantityInventoryFields({
  onChange,
  unitId,
  unitsById,
  values,
}: {
  onChange: <K extends keyof InventoryForm>(key: K, value: InventoryForm[K]) => void;
  unitId: string;
  unitsById: Map<string, Unit>;
  values: InventoryForm;
}) {
  return (
    <div className="form-grid form-grid-2">
      <FormField htmlFor="inventory-editor-quantity-on-hand" label="库存数量">
        <input
          className="input"
          id="inventory-editor-quantity-on-hand"
          min={0}
          step="any"
          type="number"
          value={values.quantity_on_hand}
          onChange={(event) => onChange("quantity_on_hand", event.target.value)}
        />
      </FormField>
      <FormField htmlFor="inventory-editor-allocated" label="已分配">
        <input
          className="input"
          id="inventory-editor-allocated"
          min={0}
          step="any"
          type="number"
          value={values.quantity_allocated}
          onChange={(event) => onChange("quantity_allocated", event.target.value)}
        />
      </FormField>
      <FormField htmlFor="inventory-editor-unit" label="单位">
        <input
          className="input"
          id="inventory-editor-unit"
          readOnly
          value={unitId ? unitLabel(unitId, unitsById) : ""}
        />
      </FormField>
    </div>
  );
}

function DeleteInventoryDialog({
  item,
  loading,
  onClose,
  onConfirm,
}: {
  item: InventoryItem | null;
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open && !loading) onClose();
      }}
      open={item !== null}
      title="删除库存"
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
        {item
          ? `确认删除“${item.asset.name} · ${inventoryItemLabel(item)}”？已分配库存将由后端拒绝删除。`
          : ""}
      </p>
    </Dialog>
  );
}

function buildBasicColumns(
  categoryById: Map<string, AssetCategory>,
  locationById: Map<string, Location>,
  unitsById: Map<string, Unit>,
): InventoryColumn[] {
  return [
    {
      key: "asset",
      label: "资产",
      locked: true,
      render: (item) => <InventoryAssetCell item={item} />,
      sortKey: "asset",
    },
    {
      key: "item",
      label: "库存",
      locked: true,
      render: (item) => <InventoryItemCell item={item} unitsById={unitsById} />,
      sortKey: "item",
    },
    {
      key: "category",
      label: "分类",
      render: (item) => categoryLabel(item.asset.category_id, categoryById),
      sortKey: "category",
    },
    {
      key: "tracking_mode",
      label: "模式",
      render: (item) => <Badge tone="accent">{trackingModeLabel(item.tracking_mode)}</Badge>,
      sortKey: "tracking_mode",
    },
    {
      key: "status",
      label: "状态",
      render: (item) => (
        <Badge tone={inventoryStatusTone(item.status)}>
          {inventoryStatusLabel(item.status)}
        </Badge>
      ),
      sortKey: "status",
    },
    {
      key: "location",
      label: "位置",
      render: (item) => locationLabel(item.location_id, locationById),
      sortKey: "location",
    },
    {
      key: "batch",
      label: "批号",
      render: (item) => item.batch_number ?? <span className="muted">未填写</span>,
      sortKey: "batch",
    },
    {
      key: "serial",
      label: "序列号",
      render: (item) => item.serial_number ?? <span className="muted">未填写</span>,
      sortKey: "serial",
    },
    {
      key: "updated_at",
      label: "更新时间",
      render: (item) => formatDate(item.updated_at),
      sortKey: "updated_at",
    },
  ];
}

function buildParameterColumns(
  parameters: AssetParameter[],
  categoryById: Map<string, AssetCategory>,
  locationById: Map<string, Location>,
  unitsById: Map<string, Unit>,
  assetDetailsById: Map<string, Asset>,
): InventoryColumn[] {
  return [
    {
      key: "asset",
      label: "资产",
      locked: true,
      render: (item) => <InventoryAssetCell item={item} />,
      sortKey: "asset",
    },
    {
      key: "item",
      label: "库存",
      locked: true,
      render: (item) => <InventoryItemCell item={item} unitsById={unitsById} />,
      sortKey: "item",
    },
    {
      key: "category",
      label: "分类",
      render: (item) => categoryLabel(item.asset.category_id, categoryById),
      sortKey: "category",
    },
    {
      key: "status",
      label: "状态",
      render: (item) => (
        <Badge tone={inventoryStatusTone(item.status)}>
          {inventoryStatusLabel(item.status)}
        </Badge>
      ),
      sortKey: "status",
    },
    {
      key: "location",
      label: "位置",
      render: (item) => locationLabel(item.location_id, locationById),
      sortKey: "location",
    },
    ...parameters.map<InventoryColumn>((parameter) => ({
      key: parameterColumnKey(parameter.parameter_type_id),
      label: parameter.name,
      render: (item) => {
        const value = findParameterValue(
          assetDetailsById.get(item.asset_id),
          parameter.parameter_type_id,
        );
        return value ? (
          <span className="asset-parameter-value">
            {formatParameterValue(value, unitsById)}
          </span>
        ) : (
          <span className="muted">未填写</span>
        );
      },
      sortKey: parameterColumnKey(parameter.parameter_type_id),
    })),
    {
      key: "updated_at",
      label: "更新时间",
      render: (item) => formatDate(item.updated_at),
      sortKey: "updated_at",
    },
  ];
}

function InventoryItemCell({
  item,
  unitsById,
}: {
  item: InventoryItem;
  unitsById: Map<string, Unit>;
}) {
  if (item.tracking_mode === "quantity") {
    return (
      <span className="asset-muted-cell">
        <strong>
          {formatNumber(item.quantity_on_hand)} {unitSymbol(item.quantity_unit_id, unitsById)}
        </strong>
        <span>
          已分配 {formatNumber(item.quantity_allocated)}{" "}
          {unitSymbol(item.quantity_unit_id, unitsById)}
        </span>
      </span>
    );
  }

  return (
    <span className="asset-name-cell">
      <strong>{inventoryItemLabel(item)}</strong>
      <span>{trackingModeLabel(item.tracking_mode)}</span>
    </span>
  );
}

function InventoryAssetCell({ item }: { item: InventoryItem }) {
  return (
    <span className="asset-name-cell">
      <strong>{item.asset.name}</strong>
      <span>{item.asset.model ?? item.asset.manufacturer ?? item.asset.asset_id}</span>
    </span>
  );
}

function itemMatchesParameterFilters(
  item: InventoryItem,
  filters: FilterForm,
  assetDetailsById: Map<string, Asset>,
  unitsById: Map<string, Unit>,
) {
  const parameterTypeId = filters.parameter_type_id;
  const keyword = filters.parameter_keyword.trim().toLowerCase();
  if (!parameterTypeId && !keyword) {
    return true;
  }

  const values = assetDetailsById.get(item.asset_id)?.parameters ?? [];
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

function sortInventoryItems(
  items: InventoryItem[],
  sort: SortState,
  categoryById: Map<string, AssetCategory>,
  locationById: Map<string, Location>,
  unitsById: Map<string, Unit>,
  assetDetailsById: Map<string, Asset>,
) {
  return [...items].sort((left, right) => {
    const leftValue = sortValue(left, sort.key, categoryById, locationById, unitsById, assetDetailsById);
    const rightValue = sortValue(right, sort.key, categoryById, locationById, unitsById, assetDetailsById);
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
  item: InventoryItem,
  key: string,
  categoryById: Map<string, AssetCategory>,
  locationById: Map<string, Location>,
  unitsById: Map<string, Unit>,
  assetDetailsById: Map<string, Asset>,
) {
  if (key.startsWith("param:")) {
    const parameterId = key.slice("param:".length);
    const value = findParameterValue(assetDetailsById.get(item.asset_id), parameterId);
    return value ? formatParameterValue(value, unitsById) : "";
  }

  switch (key) {
    case "asset":
      return `${item.asset.name} ${item.asset.model ?? ""} ${item.asset.manufacturer ?? ""}`;
    case "batch":
      return item.batch_number ?? "";
    case "category":
      return categoryLabel(item.asset.category_id, categoryById);
    case "location":
      return locationLabel(item.location_id, locationById);
    case "serial":
      return item.serial_number ?? "";
    case "status":
      return item.status;
    case "tracking_mode":
      return item.tracking_mode;
    case "updated_at":
      return new Date(item.updated_at).getTime();
    case "item":
    default:
      return item.tracking_mode === "quantity" ? item.quantity_on_hand : inventoryItemLabel(item);
  }
}

function buildLocationBreadcrumbs(locationId: string, locations: Location[]) {
  const selected = locations.find((location) => location.location_id === locationId);
  if (!selected) {
    return [{ label: "全部位置", locationId: "" }];
  }

  const breadcrumbs = [{ label: "全部位置", locationId: "" }];
  const segments = selected.path.split(".");
  for (let index = 0; index < segments.length; index += 1) {
    const path = segments.slice(0, index + 1).join(".");
    const location = locations.find((candidate) => candidate.path === path);
    if (location) {
      breadcrumbs.push({ label: location.name, locationId: location.location_id });
    }
  }
  return breadcrumbs;
}

function findParameterValue(asset: Asset | undefined, parameterId: string) {
  return asset?.parameters?.find((value) => value.parameter_type_id === parameterId);
}

function inventoryItemLabel(item: InventoryItem) {
  return item.serial_number ?? item.batch_number ?? item.inventory_item_id;
}

function unitSymbol(unitId: string, unitsById: Map<string, Unit>) {
  return unitsById.get(unitId)?.symbol ?? "";
}

function canAttachToCreatedInventoryItem(values: InventoryForm, asset: Asset) {
  if (asset.tracking_mode === "quantity") {
    return true;
  }
  if (values.serial_mode === "count") {
    return parsePositiveNumber(values.count) === 1;
  }
  return splitSerialNumbers(values.serial_numbers).length === 1;
}

function buildCreatePayload(
  values: InventoryForm,
  asset: Asset,
): { ok: true; payload: CreateInventoryItemsPayload } | { message: string; ok: false } {
  const base = {
    batch_number: optionalText(values.batch_number),
    internal_notes: optionalText(values.internal_notes),
    location_id: values.location_id || null,
    public_notes: optionalText(values.public_notes),
    status: values.status,
  };

  if (asset.tracking_mode === "serialized") {
    if (values.serial_mode === "count") {
      const count = parsePositiveNumber(values.count);
      if (!count || !Number.isInteger(count)) {
        return { message: "请输入正整数创建数量", ok: false };
      }
      return { ok: true, payload: { ...base, count } };
    }

    const serialNumbers = splitSerialNumbers(values.serial_numbers);
    if (serialNumbers.length === 0) {
      return { message: "请输入至少一个序列号", ok: false };
    }
    return { ok: true, payload: { ...base, serial_numbers: serialNumbers } };
  }

  const quantityOnHand = parseNonNegativeNumber(values.quantity_on_hand);
  const quantityAllocated = parseNonNegativeNumber(values.quantity_allocated || "0");
  if (quantityOnHand === null) {
    return { message: "请输入有效库存数量", ok: false };
  }
  if (quantityAllocated === null) {
    return { message: "请输入有效已分配数量", ok: false };
  }
  if (quantityAllocated > quantityOnHand) {
    return { message: "已分配数量不能超过库存数量", ok: false };
  }

  return {
    ok: true,
    payload: {
      ...base,
      quantity_allocated: quantityAllocated,
      quantity_on_hand: quantityOnHand,
    },
  };
}

function buildUpdatePayload(
  values: InventoryForm,
  item: InventoryItem,
): { ok: true; payload: UpdateInventoryItemPayload } | { message: string; ok: false } {
  const payload: UpdateInventoryItemPayload = {
    batch_number: optionalText(values.batch_number),
    internal_notes: optionalText(values.internal_notes),
    location_id: values.location_id || null,
    public_notes: optionalText(values.public_notes),
    status: values.status,
  };

  if (item.tracking_mode === "serialized") {
    const serialNumber = values.serial_number.trim();
    if (!serialNumber) {
      return { message: "序列号不能为空", ok: false };
    }
    payload.serial_number = serialNumber;
    return { ok: true, payload };
  }

  const quantityOnHand = parseNonNegativeNumber(values.quantity_on_hand);
  const quantityAllocated = parseNonNegativeNumber(values.quantity_allocated || "0");
  if (quantityOnHand === null) {
    return { message: "请输入有效库存数量", ok: false };
  }
  if (quantityAllocated === null) {
    return { message: "请输入有效已分配数量", ok: false };
  }
  if (quantityAllocated > quantityOnHand) {
    return { message: "已分配数量不能超过库存数量", ok: false };
  }
  payload.quantity_allocated = quantityAllocated;
  payload.quantity_on_hand = quantityOnHand;
  return { ok: true, payload };
}

function formFromInventoryItem(item: InventoryItem): InventoryForm {
  return {
    ...emptyInventoryForm(),
    batch_number: item.batch_number ?? "",
    internal_notes: item.internal_notes ?? "",
    location_id: item.location_id ?? "",
    public_notes: item.public_notes ?? "",
    quantity_allocated: String(item.quantity_allocated),
    quantity_on_hand: String(item.quantity_on_hand),
    quantity_unit_id: item.asset.default_unit_id,
    serial_number: item.serial_number ?? "",
    status: item.status,
  };
}

function emptyInventoryForm(): InventoryForm {
  return {
    asset_id: "",
    batch_number: "",
    count: "1",
    internal_notes: "",
    location_id: "",
    public_notes: "",
    quantity_allocated: "0",
    quantity_on_hand: "",
    quantity_unit_id: "",
    serial_mode: "serials",
    serial_number: "",
    serial_numbers: "",
    status: "available",
  };
}

function emptyFilters(categoryId = "", exactCategory = false, locationId = ""): FilterForm {
  return {
    batch_number: "",
    category_id: categoryId,
    exact_category: exactCategory,
    has_batch: "all",
    has_location: "all",
    keyword: "",
    location_id: locationId,
    parameter_keyword: "",
    parameter_type_id: "",
    serial_number: "",
    status: "all",
    tracking_mode: "all",
  };
}

function inventoryStatusOptions() {
  return [
    { label: "可用", value: "available" },
    { label: "预留", value: "reserved" },
    { label: "退役", value: "retired" },
    { label: "丢失", value: "lost" },
    { label: "已消耗", value: "consumed" },
  ];
}

function splitSerialNumbers(value: string) {
  return value
    .split(/[\n,]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function parsePositiveNumber(value: string) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : null;
}

function parseNonNegativeNumber(value: string) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : null;
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
