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
  Search,
  Settings2,
  Trash2,
  X,
} from "lucide-react";
import {
  type Dispatch,
  type FormEvent,
  type ReactNode,
  type SetStateAction,
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
  assetDetailPath,
  laboratoryDetailScopeCacheKey,
} from "../federation/scope";
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
  parameter_filters: ParameterFilterDraft[];
  serial_number: string;
  status: "all" | InventoryStatus;
};

type InventoryFilterKey =
  | "category"
  | "status"
  | "location"
  | "serial_number"
  | "batch_number"
  | "has_batch"
  | "has_location";

type ParameterFilterDraft = {
  boolean: "all" | "true" | "false";
  date_end: string;
  date_start: string;
  id: string;
  number_max: string;
  number_min: string;
  option_id: string;
  parameter_type_id: string;
  range_end: string;
  range_start: string;
  text: string;
  unit_id: string;
};

type SerializedParameterFilter = {
  boolean?: boolean;
  date_end?: string;
  date_start?: string;
  number_max?: number;
  number_min?: number;
  option_id?: string;
  parameter_type_id: string;
  range_end?: number;
  range_start?: number;
  text?: string;
  unit_id?: string;
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
  | { asset?: Asset; mode: "create" }
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
const INVENTORY_FILTER_OPTIONS: Array<{ label: string; value: InventoryFilterKey }> = [
  { label: "资产分类", value: "category" },
  { label: "库存状态", value: "status" },
  { label: "位置", value: "location" },
  { label: "序列号", value: "serial_number" },
  { label: "批号", value: "batch_number" },
  { label: "是否有批号", value: "has_batch" },
  { label: "是否有位置", value: "has_location" },
];

export function InventoryPage() {
  const {
    canManageSelectedLaboratoryAssets,
    selectedDataScope,
    selectedLaboratoryId,
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
  const [filterDialogOpen, setFilterDialogOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchValue, setSearchValue] = useState("");
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
    scope: selectedDataScope,
  });
  const parametersQuery = useAssetParameters({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    scope: selectedDataScope,
  });
  const locationsQuery = useLocations({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    scope: selectedDataScope,
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
  const serializedParameterFilters = useMemo(
    () => serializeParameterFilters(filters.parameter_filters, parameters),
    [filters.parameter_filters, parameters],
  );
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
  }, [selectedDataScope]);

  useEffect(() => {
    setSearchValue(filters.keyword);
  }, [filters.keyword]);

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
      parameter_filters: serializedParameterFilters,
      serial_number: optional(filters.serial_number),
      status: filters.status === "all" ? undefined : filters.status,
    }),
    [filters, offset, serializedParameterFilters],
  );
  const inventoryQuery = useInventoryItems({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    query,
    scope: selectedDataScope,
  });
  const response = inventoryQuery.data;
  const total = response?.total ?? 0;
  const page = Math.floor(offset / PAGE_SIZE) + 1;
  const maxPage = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const parameterFilterActive = filters.parameter_filters.length > 0;
  const needsAssetDetails = viewMode === "parameters";
  const currentPageAssetIds = useMemo(
    () => Array.from(new Set((response?.items ?? []).map((item) => item.asset_id))),
    [response?.items],
  );
  const assetDetailQueries = useQueries({
    queries: currentPageAssetIds.map((assetId) => ({
      enabled: Boolean(apiBaseUrl) && needsAssetDetails,
      queryKey: assetQueryKeys.detail(
        apiBaseUrl,
        laboratoryDetailScopeCacheKey(selectedDataScope),
        assetId,
        true,
      ),
      queryFn: async () => {
        const client = createApiClient(apiBaseUrl);
        return assetSchema.parse(
          await client.get(assetDetailPath(selectedDataScope, assetId), {
            include: "parameters",
          }),
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

  const visibleItems = useMemo(
    () =>
      sortInventoryItems(
        response?.items ?? [],
        sort,
        categoryById,
        locationById,
        unitsById,
        assetDetailsById,
      ),
    [assetDetailsById, categoryById, locationById, response?.items, sort, unitsById],
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

  function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const keyword = searchValue.trim();
    const next = { ...filters, keyword };
    setOffset(0);
    setFilters(next);
    setDraftFilters(next);
  }

  function clearSearch() {
    const next = { ...filters, keyword: "" };
    setSearchValue("");
    setOffset(0);
    setFilters(next);
    setDraftFilters(next);
  }

  function applyDraftFilters() {
    const validation = validateParameterFilters(draftFilters.parameter_filters, parameters);
    if (!validation.ok) {
      toast.error({ title: validation.message });
      return;
    }
    const next = { ...draftFilters, keyword: filters.keyword };
    setOffset(0);
    setFilters(next);
    setDraftFilters(next);
    syncInventorySearch(next.category_id, next.exact_category, next.location_id);
    setFilterDialogOpen(false);
  }

  function resetFilters() {
    const next = { ...emptyFilters(), keyword: filters.keyword };
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

  function handleSort(key: string) {
    setSort((current) =>
      current.key === key
        ? { direction: current.direction === "asc" ? "desc" : "asc", key }
        : { direction: "asc", key },
    );
  }

  function applyColumns(nextColumns: Set<string>) {
    const setter =
      viewMode === "basic" ? setVisibleBasicColumns : setVisibleParameterColumns;
    setter(nextColumns);
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
        <div className="panel-header asset-table-header">
          <div>
            <h2 className="panel-title">库存列表</h2>
            <p className="panel-description">
              第 {page} / {maxPage} 页，共 {total} 条
              {parameterFilterActive ? "，已应用参数过滤" : ""}
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
            {searchOpen ? (
              <form className="list-search" onSubmit={submitSearch}>
                <input
                  aria-label="搜索库存"
                  className="input list-search-input"
                  placeholder="搜索库存"
                  value={searchValue}
                  onChange={(event) => setSearchValue(event.target.value)}
                />
                <Button size="icon" variant="ghost" aria-label="应用搜索" type="submit">
                  <Search size={16} />
                </Button>
                <Button size="icon" variant="ghost" aria-label="清空搜索" onClick={clearSearch}>
                  <X size={16} />
                </Button>
              </form>
            ) : (
              <Button
                size="icon"
                variant={filters.keyword ? "default" : "ghost"}
                aria-label="搜索"
                onClick={() => setSearchOpen(true)}
              >
                <Search size={16} />
              </Button>
            )}
            <Button
              size="icon"
              variant={hasActiveInventoryFilters(filters) ? "default" : "ghost"}
              aria-label="过滤条件"
              onClick={() => {
                setDraftFilters(filters);
                setFilterDialogOpen(true);
              }}
            >
              <Filter size={16} />
            </Button>
            <ColumnSelector
              columns={activeColumns}
              visibleColumns={visibleColumnKeys}
              onApply={applyColumns}
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
      <InventoryFilterDialog
        categories={categories}
        filters={draftFilters}
        locations={locations}
        open={filterDialogOpen}
        parameters={parameters}
        units={units}
        onApply={applyDraftFilters}
        onChange={setDraftFilters}
        onOpenChange={setFilterDialogOpen}
        onReset={resetFilters}
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

function InventoryFilterDialog({
  categories,
  filters,
  locations,
  onApply,
  onChange,
  onOpenChange,
  onReset,
  open,
  parameters,
  units,
}: {
  categories: AssetCategory[];
  filters: FilterForm;
  locations: Location[];
  onApply: () => void;
  onChange: (value: FilterForm | ((current: FilterForm) => FilterForm)) => void;
  onOpenChange: (open: boolean) => void;
  onReset: () => void;
  open: boolean;
  parameters: AssetParameter[];
  units: Unit[];
}) {
  const [activeKeys, setActiveKeys] = useState<Set<InventoryFilterKey>>(
    () => activeInventoryFilterKeys(filters),
  );
  const [conditionToAdd, setConditionToAdd] = useState<InventoryFilterKey | "none">("none");

  useEffect(() => {
    if (open) {
      setActiveKeys(activeInventoryFilterKeys(filters));
      setConditionToAdd("none");
    }
  }, [filters, open]);

  const availableOptions = INVENTORY_FILTER_OPTIONS.filter(
    (option) => !activeKeys.has(option.value),
  );

  function update<K extends keyof FilterForm>(key: K, value: FilterForm[K]) {
    onChange((current) => ({ ...current, [key]: value }));
  }

  function removeCondition(key: InventoryFilterKey) {
    setActiveKeys((current) => {
      const next = new Set(current);
      next.delete(key);
      return next;
    });
    onChange((current) => resetInventoryFilterKey(current, key));
  }

  function updateParameterFilter(id: string, patch: Partial<ParameterFilterDraft>) {
    onChange((current) => ({
      ...current,
      parameter_filters: current.parameter_filters.map((filter) =>
        filter.id === id ? { ...filter, ...patch } : filter,
      ),
    }));
  }

  function removeParameterFilter(id: string) {
    onChange((current) => ({
      ...current,
      parameter_filters: current.parameter_filters.filter((filter) => filter.id !== id),
    }));
  }

  return (
    <Dialog
      open={open}
      title="过滤库存"
      description="逐项添加条件；多个条件会同时生效。"
      onOpenChange={onOpenChange}
      footer={
        <>
          <Button onClick={onReset}>
            <RotateCcw size={15} />
            重置
          </Button>
          <Button onClick={() => onOpenChange(false)}>取消</Button>
          <Button variant="primary" onClick={onApply}>
            应用过滤
          </Button>
        </>
      }
    >
      <div className="filter-dialog-add-row">
        <Select
          label="添加过滤条件"
          options={[
            { label: "选择条件", value: "none" },
            ...availableOptions.map((option) => ({
              label: option.label,
              value: option.value,
            })),
          ]}
          value={conditionToAdd}
          onValueChange={(value) =>
            setConditionToAdd(value as InventoryFilterKey | "none")
          }
        />
        <Button
          disabled={conditionToAdd === "none"}
          onClick={() => {
            if (conditionToAdd === "none") return;
            setActiveKeys((current) => new Set(current).add(conditionToAdd));
            setConditionToAdd("none");
          }}
        >
          <Plus size={15} />
          添加条件
        </Button>
      </div>

      <div className="filter-condition-list">
        {[...activeKeys].map((key) => (
          <div className="filter-condition-item" key={key}>
            <div className="filter-condition-header">
              <strong>{inventoryFilterLabel(key)}</strong>
              <Button size="icon" variant="ghost" aria-label={`移除${inventoryFilterLabel(key)}`} onClick={() => removeCondition(key)}>
                <X size={15} />
              </Button>
            </div>
            {key === "category" ? (
              <>
                <Select
                  label="资产分类"
                  options={[
                    { label: "全部分类", value: "all" },
                    ...categories.map((category) => ({
                      label: `${"　".repeat(category.depth)}${category.name}`,
                      value: category.category_id,
                    })),
                  ]}
                  value={filters.category_id || "all"}
                  onValueChange={(value) =>
                    update("category_id", value === "all" ? "" : value)
                  }
                />
                <label className="checkbox-field">
                  <input
                    checked={filters.exact_category}
                    disabled={!filters.category_id}
                    type="checkbox"
                    onChange={(event) => update("exact_category", event.target.checked)}
                  />
                  <span>
                    <strong>精确分类</strong>
                    <small>关闭时包含所选分类的所有子分类库存。</small>
                  </span>
                </label>
              </>
            ) : null}
            {key === "status" ? (
              <Select
                label="库存状态"
                options={[
                  { label: "全部状态", value: "all" },
                  ...inventoryStatusOptions(),
                ]}
                value={filters.status}
                onValueChange={(value) => update("status", value as FilterForm["status"])}
              />
            ) : null}
            {key === "location" ? (
              <Select
                label="位置"
                options={[
                  { label: "全部位置", value: "all" },
                  ...locations.map((location) => ({
                    label: `${"　".repeat(location.depth)}${location.name}`,
                    value: location.location_id,
                  })),
                ]}
                value={filters.location_id || "all"}
                onValueChange={(value) =>
                  update("location_id", value === "all" ? "" : value)
                }
              />
            ) : null}
            {key === "serial_number" ? (
              <FormField htmlFor="inventory-dialog-serial" label="序列号">
                <input
                  className="input"
                  id="inventory-dialog-serial"
                  value={filters.serial_number}
                  onChange={(event) => update("serial_number", event.target.value)}
                />
              </FormField>
            ) : null}
            {key === "batch_number" ? (
              <FormField htmlFor="inventory-dialog-batch" label="批号">
                <input
                  className="input"
                  id="inventory-dialog-batch"
                  value={filters.batch_number}
                  onChange={(event) => update("batch_number", event.target.value)}
                />
              </FormField>
            ) : null}
            {key === "has_batch" ? (
              <Select
                label="是否有批号"
                options={[
                  { label: "全部", value: "all" },
                  { label: "有批号", value: "true" },
                  { label: "无批号", value: "false" },
                ]}
                value={filters.has_batch}
                onValueChange={(value) =>
                  update("has_batch", value as FilterForm["has_batch"])
                }
              />
            ) : null}
            {key === "has_location" ? (
              <Select
                label="是否有位置"
                options={[
                  { label: "全部", value: "all" },
                  { label: "有位置", value: "true" },
                  { label: "无位置", value: "false" },
                ]}
                value={filters.has_location}
                onValueChange={(value) =>
                  update("has_location", value as FilterForm["has_location"])
                }
              />
            ) : null}
          </div>
        ))}

        {filters.parameter_filters.map((filter, index) => (
          <div className="filter-condition-item" key={filter.id}>
            <div className="filter-condition-header">
              <strong>资产参数 {index + 1}</strong>
              <Button
                size="icon"
                variant="ghost"
                aria-label={`移除资产参数 ${index + 1}`}
                onClick={() => removeParameterFilter(filter.id)}
              >
                <X size={15} />
              </Button>
            </div>
            <Select
              label="参数"
              options={[
                { label: "选择参数", value: "none" },
                ...parameters.map((parameter) => ({
                  label: parameter.name,
                  value: parameter.parameter_type_id,
                })),
              ]}
              value={filter.parameter_type_id || "none"}
              onValueChange={(value) =>
                updateParameterFilter(filter.id, {
                  ...emptyParameterFilterDraft(),
                  id: filter.id,
                  parameter_type_id: value === "none" ? "" : value,
                })
              }
            />
            <ParameterFilterFields
              filter={filter}
              parameters={parameters}
              units={units}
              onChange={(patch) => updateParameterFilter(filter.id, patch)}
            />
          </div>
        ))}
      </div>

      <Button
        onClick={() =>
          onChange((current) => ({
            ...current,
            parameter_filters: [
              ...current.parameter_filters,
              emptyParameterFilterDraft(),
            ],
          }))
        }
      >
        <Plus size={15} />
        添加参数条件
      </Button>
    </Dialog>
  );
}

function ParameterFilterFields({
  filter,
  onChange,
  parameters,
  units,
}: {
  filter: ParameterFilterDraft;
  onChange: (patch: Partial<ParameterFilterDraft>) => void;
  parameters: AssetParameter[];
  units: Unit[];
}) {
  const parameter = parameters.find(
    (item) => item.parameter_type_id === filter.parameter_type_id,
  );
  if (!parameter) {
    return null;
  }
  const unitOptions = units.filter((unit) => unit.dimension === parameter.unit_dimension);
  const unitSelect =
    parameter.data_type === "number" || parameter.data_type === "range" ? (
      <Select
        label="单位"
        options={[
          { label: "默认单位", value: "default" },
          ...unitOptions.map((unit) => ({
            label: `${unit.name} (${unit.symbol})`,
            value: unit.unit_id,
          })),
        ]}
        value={filter.unit_id || "default"}
        onValueChange={(value) => onChange({ unit_id: value === "default" ? "" : value })}
      />
    ) : null;

  if (parameter.data_type === "text") {
    return (
      <FormField htmlFor={`inventory-parameter-filter-text-${filter.id}`} label="包含文本">
        <input
          className="input"
          id={`inventory-parameter-filter-text-${filter.id}`}
          value={filter.text}
          onChange={(event) => onChange({ text: event.target.value })}
        />
      </FormField>
    );
  }
  if (parameter.data_type === "number") {
    return (
      <div className="form-grid form-grid-2">
        <FormField htmlFor={`inventory-parameter-filter-number-min-${filter.id}`} label="最小值">
          <input
            className="input"
            id={`inventory-parameter-filter-number-min-${filter.id}`}
            type="number"
            step="any"
            value={filter.number_min}
            onChange={(event) => onChange({ number_min: event.target.value })}
          />
        </FormField>
        <FormField htmlFor={`inventory-parameter-filter-number-max-${filter.id}`} label="最大值">
          <input
            className="input"
            id={`inventory-parameter-filter-number-max-${filter.id}`}
            type="number"
            step="any"
            value={filter.number_max}
            onChange={(event) => onChange({ number_max: event.target.value })}
          />
        </FormField>
        {unitSelect}
      </div>
    );
  }
  if (parameter.data_type === "range") {
    return (
      <div className="form-grid form-grid-2">
        <FormField htmlFor={`inventory-parameter-filter-range-start-${filter.id}`} label="范围起点">
          <input
            className="input"
            id={`inventory-parameter-filter-range-start-${filter.id}`}
            type="number"
            step="any"
            value={filter.range_start}
            onChange={(event) => onChange({ range_start: event.target.value })}
          />
        </FormField>
        <FormField htmlFor={`inventory-parameter-filter-range-end-${filter.id}`} label="范围终点">
          <input
            className="input"
            id={`inventory-parameter-filter-range-end-${filter.id}`}
            type="number"
            step="any"
            value={filter.range_end}
            onChange={(event) => onChange({ range_end: event.target.value })}
          />
        </FormField>
        {unitSelect}
      </div>
    );
  }
  if (parameter.data_type === "boolean") {
    return (
      <Select
        label="布尔值"
        options={[
          { label: "选择值", value: "all" },
          { label: "是", value: "true" },
          { label: "否", value: "false" },
        ]}
        value={filter.boolean}
        onValueChange={(value) => onChange({ boolean: value as ParameterFilterDraft["boolean"] })}
      />
    );
  }
  if (parameter.data_type === "date") {
    return (
      <div className="form-grid form-grid-2">
        <FormField htmlFor={`inventory-parameter-filter-date-start-${filter.id}`} label="开始日期">
          <input
            className="input"
            id={`inventory-parameter-filter-date-start-${filter.id}`}
            type="date"
            value={filter.date_start}
            onChange={(event) => onChange({ date_start: event.target.value })}
          />
        </FormField>
        <FormField htmlFor={`inventory-parameter-filter-date-end-${filter.id}`} label="结束日期">
          <input
            className="input"
            id={`inventory-parameter-filter-date-end-${filter.id}`}
            type="date"
            value={filter.date_end}
            onChange={(event) => onChange({ date_end: event.target.value })}
          />
        </FormField>
      </div>
    );
  }
  if (parameter.data_type === "enum") {
    return (
      <Select
        label="选项"
        options={[
          { label: "选择选项", value: "none" },
          ...parameter.options.map((option) => ({
              label: option.label,
              value: option.option_id,
            })),
        ]}
        value={filter.option_id || "none"}
        onValueChange={(value) => onChange({ option_id: value === "none" ? "" : value })}
      />
    );
  }
  return null;
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
          <Link to={breadcrumb.locationId ? `/inventory?location_id=${breadcrumb.locationId}` : "/inventory"}>
            {breadcrumb.label}
          </Link>
        </span>
      ))}
    </nav>
  );
}

function ColumnSelector({
  columns,
  onApply,
  visibleColumns,
}: {
  columns: InventoryColumn[];
  onApply: (columns: Set<string>) => void;
  visibleColumns: Set<string>;
}) {
  const [open, setOpen] = useState(false);
  const [draftColumns, setDraftColumns] = useState<Set<string>>(() => new Set(visibleColumns));

  function openDialog() {
    setDraftColumns(new Set(visibleColumns));
    setOpen(true);
  }

  function toggleDraftColumn(key: string, visible: boolean) {
    setDraftColumns((current) => {
      const next = new Set(current);
      if (visible) {
        next.add(key);
      } else {
        next.delete(key);
      }
      return next;
    });
  }

  return (
    <>
      <Button aria-label="选择列" size="icon" variant="ghost" onClick={openDialog}>
        <Settings2 size={16} />
      </Button>
      <Dialog
        open={open}
        title="选择列"
        description="勾选本视图要显示的列。"
        onOpenChange={setOpen}
        footer={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button
              variant="primary"
              onClick={() => {
                onApply(draftColumns);
                setOpen(false);
              }}
            >
              确认
            </Button>
          </>
        }
      >
        <div className="column-dialog-list">
          {columns.map((column) => (
            <label className="checkbox-field" key={column.key}>
              <input
                checked={column.locked || draftColumns.has(column.key)}
                disabled={column.locked}
                type="checkbox"
                onChange={(event) =>
                  toggleDraftColumn(column.key, event.currentTarget.checked)
                }
              />
              <span>
                <strong>{column.label}</strong>
                {column.locked ? <small>固定显示</small> : null}
              </span>
            </label>
          ))}
        </div>
      </Dialog>
    </>
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
  const fixedAsset = editor?.mode === "create" ? editor.asset : undefined;
  const editingItem = editor?.mode === "edit" ? editor.item : null;
  const [assetSearch, setAssetSearch] = useState("");
  const [values, setValues] = useState<InventoryForm>(() => emptyInventoryForm());
  const [pendingAttachments, setPendingAttachments] = useState<PendingAttachment[]>([]);
  const [serialAttachments, setSerialAttachments] = useState<Record<string, PendingAttachment[]>>(
    {},
  );
  const unitsById = useMemo(() => mapById(units, "unit_id"), [units]);
  const assetQuery = useMemo<AssetQuery>(
    () => ({
      keyword: optional(assetSearch),
      limit: 50,
      offset: 0,
    }),
    [assetSearch],
  );
  const assetsQuery = useAssets({
    enabled: open && isCreate && !fixedAsset && Boolean(laboratoryId),
    laboratoryId,
    query: assetQuery,
  });
  const assetOptions = assetsQuery.data?.items ?? [];
  const selectedAsset = isCreate
    ? fixedAsset ?? assetOptions.find((asset) => asset.asset_id === values.asset_id)
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
      setSerialAttachments({});
      return;
    }

    if (editor.mode === "create") {
      setValues({
        ...emptyInventoryForm(),
        asset_id: editor.asset?.asset_id ?? "",
        quantity_unit_id: editor.asset?.default_unit_id ?? "",
      });
      setPendingAttachments([]);
      setSerialAttachments({});
      return;
    }

    setValues(formFromInventoryItem(editor.item));
    setPendingAttachments([]);
    setSerialAttachments({});
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
      const serialNumbers =
        selectedAsset.tracking_mode === "serialized" && values.serial_mode === "serials"
          ? splitSerialNumbers(values.serial_numbers)
          : [];
      const serialAttachmentClaims = buildSerialItemAttachmentClaims(
        serialNumbers,
        serialAttachments,
      );
      if (!serialAttachmentClaims.ok) {
        toast.error({ title: serialAttachmentClaims.message });
        return;
      }
      const attachmentClaims = serialAttachmentClaims.hasClaims
        ? attachmentClaimsFromPending([])
        : attachmentClaimsFromPending(pendingAttachments);
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
          payload: buildInventoryCreateRequestPayload(
            payloadResult.payload,
            attachmentClaims.claims,
            serialAttachmentClaims.items,
            serialAttachmentClaims.hasClaims,
          ),
        },
        {
          onError: (error) =>
            toast.error({ title: "创建库存失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            setPendingAttachments([]);
            setSerialAttachments({});
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
            {!fixedAsset ? (
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
              </>
            ) : null}
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

        {isCreate && trackingMode === "serialized" && values.serial_mode === "serials" ? (
          <SerialItemAttachmentEditors
            disabled={isSaving}
            laboratoryId={laboratoryId}
            pendingBySerial={serialAttachments}
            serialNumbers={splitSerialNumbers(values.serial_numbers)}
            onChange={setSerialAttachments}
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
            {isCreate && !(trackingMode === "serialized" && values.serial_mode === "serials") ? (
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

function SerialItemAttachmentEditors({
  disabled,
  laboratoryId,
  onChange,
  pendingBySerial,
  serialNumbers,
}: {
  disabled: boolean;
  laboratoryId: string;
  onChange: Dispatch<SetStateAction<Record<string, PendingAttachment[]>>>;
  pendingBySerial: Record<string, PendingAttachment[]>;
  serialNumbers: string[];
}) {
  if (serialNumbers.length === 0) {
    return null;
  }

  function updateSerialAttachments(
    serialNumber: string,
    update: SetStateAction<PendingAttachment[]>,
  ) {
    onChange((current) => {
      const previous = current[serialNumber] ?? [];
      const nextValue =
        typeof update === "function"
          ? (update as (current: PendingAttachment[]) => PendingAttachment[])(previous)
          : update;
      return { ...current, [serialNumber]: nextValue };
    });
  }

  return (
    <div className="serial-attachment-list">
      {serialNumbers.map((serialNumber) => (
        <div className="serial-attachment-item" key={serialNumber}>
          <div className="serial-attachment-title">
            <strong>{serialNumber}</strong>
            <span>独立附件</span>
          </div>
          <PendingAttachmentUploader
            disabled={disabled}
            laboratoryId={laboratoryId}
            pendingAttachments={pendingBySerial[serialNumber] ?? []}
            onChange={(update) => updateSerialAttachments(serialNumber, update)}
          />
        </div>
      ))}
    </div>
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
      <span>{item.batch_number ? `批号 ${item.batch_number}` : "单件库存"}</span>
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
  const byId = mapById(locations, "location_id");
  const chain: Location[] = [];
  let current: Location | undefined = selected;
  const seen = new Set<string>();
  while (current && !seen.has(current.location_id)) {
    seen.add(current.location_id);
    chain.unshift(current);
    current = current.parent_location_id ? byId.get(current.parent_location_id) : undefined;
  }
  for (const location of chain) {
    breadcrumbs.push({ label: location.name, locationId: location.location_id });
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

function buildSerialItemAttachmentClaims(
  serialNumbers: string[],
  pendingBySerial: Record<string, PendingAttachment[]>,
):
  | {
      hasClaims: boolean;
      items: NonNullable<CreateInventoryItemsPayload["serial_items"]>;
      ok: true;
    }
  | { message: string; ok: false } {
  const items: NonNullable<CreateInventoryItemsPayload["serial_items"]> = [];
  let hasClaims = false;
  for (const serialNumber of serialNumbers) {
    const claims = attachmentClaimsFromPending(pendingBySerial[serialNumber] ?? []);
    if (!claims.ok) {
      return { message: `${serialNumber}: ${claims.message}`, ok: false };
    }
    hasClaims = hasClaims || claims.claims.length > 0;
    items.push({
      attachments: claims.claims.length > 0 ? claims.claims : undefined,
      serial_number: serialNumber,
    });
  }
  return { hasClaims, items, ok: true };
}

function buildInventoryCreateRequestPayload(
  payload: CreateInventoryItemsPayload,
  attachments: NonNullable<CreateInventoryItemsPayload["attachments"]>,
  serialItems: NonNullable<CreateInventoryItemsPayload["serial_items"]>,
  useSerialItems: boolean,
): CreateInventoryItemsPayload {
  if (useSerialItems) {
    return {
      ...payload,
      attachments: undefined,
      serial_items: serialItems,
      serial_numbers: undefined,
    };
  }
  return {
    ...payload,
    attachments: attachments.length > 0 ? attachments : undefined,
  };
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
    parameter_filters: [],
    serial_number: "",
    status: "all",
  };
}

function activeInventoryFilterKeys(filters: FilterForm) {
  const keys = new Set<InventoryFilterKey>();
  if (filters.category_id || filters.exact_category) keys.add("category");
  if (filters.status !== "all") keys.add("status");
  if (filters.location_id) keys.add("location");
  if (filters.serial_number.trim()) keys.add("serial_number");
  if (filters.batch_number.trim()) keys.add("batch_number");
  if (filters.has_batch !== "all") keys.add("has_batch");
  if (filters.has_location !== "all") keys.add("has_location");
  return keys;
}

function hasActiveInventoryFilters(filters: FilterForm) {
  return activeInventoryFilterKeys(filters).size > 0 || filters.parameter_filters.length > 0;
}

function inventoryFilterLabel(key: InventoryFilterKey) {
  return INVENTORY_FILTER_OPTIONS.find((option) => option.value === key)?.label ?? key;
}

function resetInventoryFilterKey(filters: FilterForm, key: InventoryFilterKey): FilterForm {
  switch (key) {
    case "category":
      return { ...filters, category_id: "", exact_category: false };
    case "status":
      return { ...filters, status: "all" };
    case "location":
      return { ...filters, location_id: "" };
    case "serial_number":
      return { ...filters, serial_number: "" };
    case "batch_number":
      return { ...filters, batch_number: "" };
    case "has_batch":
      return { ...filters, has_batch: "all" };
    case "has_location":
      return { ...filters, has_location: "all" };
    default:
      return filters;
  }
}

function emptyParameterFilterDraft(): ParameterFilterDraft {
  return {
    boolean: "all",
    date_end: "",
    date_start: "",
    id: `parameter-filter-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    number_max: "",
    number_min: "",
    option_id: "",
    parameter_type_id: "",
    range_end: "",
    range_start: "",
    text: "",
    unit_id: "",
  };
}

function validateParameterFilters(
  filters: ParameterFilterDraft[],
  parameters: AssetParameter[],
): { ok: true } | { message: string; ok: false } {
  for (const filter of filters) {
    const parameter = parameters.find(
      (item) => item.parameter_type_id === filter.parameter_type_id,
    );
    if (!parameter) return { message: "请选择每个参数过滤条件的参数。", ok: false };
    if (parameter.data_type === "text" && !filter.text.trim()) {
      return { message: `${parameter.name} 需要填写包含文本。`, ok: false };
    }
    if (parameter.data_type === "number") {
      const min = optionalNumber(filter.number_min);
      const max = optionalNumber(filter.number_max);
      if (!min.ok || !max.ok) return { message: `${parameter.name} 数值范围无效。`, ok: false };
      if (min.value === null && max.value === null) {
        return { message: `${parameter.name} 至少填写一个数值边界。`, ok: false };
      }
      if (min.value !== null && max.value !== null && min.value > max.value) {
        return { message: `${parameter.name} 最小值不能大于最大值。`, ok: false };
      }
    }
    if (parameter.data_type === "range") {
      const start = optionalNumber(filter.range_start);
      const end = optionalNumber(filter.range_end);
      if (!start.ok || !end.ok || start.value === null || end.value === null) {
        return { message: `${parameter.name} 需要填写完整范围。`, ok: false };
      }
      if (start.value > end.value) {
        return { message: `${parameter.name} 范围起点不能大于终点。`, ok: false };
      }
    }
    if (parameter.data_type === "boolean" && filter.boolean === "all") {
      return { message: `${parameter.name} 需要选择布尔值。`, ok: false };
    }
    if (parameter.data_type === "date") {
      if (!filter.date_start && !filter.date_end) {
        return { message: `${parameter.name} 至少填写一个日期边界。`, ok: false };
      }
      if (filter.date_start && filter.date_end && filter.date_start > filter.date_end) {
        return { message: `${parameter.name} 开始日期不能晚于结束日期。`, ok: false };
      }
    }
    if (parameter.data_type === "enum" && !filter.option_id) {
      return { message: `${parameter.name} 需要选择选项。`, ok: false };
    }
  }
  return { ok: true };
}

function serializeParameterFilters(
  filters: ParameterFilterDraft[],
  parameters: AssetParameter[],
) {
  const payload: SerializedParameterFilter[] = [];
  for (const filter of filters) {
    const parameter = parameters.find(
      (item) => item.parameter_type_id === filter.parameter_type_id,
    );
    if (!parameter) continue;
    const base = {
      parameter_type_id: filter.parameter_type_id,
      unit_id: filter.unit_id || undefined,
    };
    if (parameter.data_type === "text" && filter.text.trim()) {
      payload.push({ parameter_type_id: filter.parameter_type_id, text: filter.text.trim() });
    }
    if (parameter.data_type === "number") {
      const min = optionalNumber(filter.number_min);
      const max = optionalNumber(filter.number_max);
      if (min.ok && max.ok && (min.value !== null || max.value !== null)) {
        payload.push({
          ...base,
          number_max: max.value ?? undefined,
          number_min: min.value ?? undefined,
        });
      }
    }
    if (parameter.data_type === "range") {
      const start = optionalNumber(filter.range_start);
      const end = optionalNumber(filter.range_end);
      if (start.ok && end.ok && start.value !== null && end.value !== null) {
        payload.push({
          ...base,
          range_end: end.value,
          range_start: start.value,
        });
      }
    }
    if (parameter.data_type === "boolean" && filter.boolean !== "all") {
      payload.push({
        parameter_type_id: filter.parameter_type_id,
        boolean: filter.boolean === "true",
      });
    }
    if (parameter.data_type === "date" && (filter.date_start || filter.date_end)) {
      payload.push({
        parameter_type_id: filter.parameter_type_id,
        date_end: filter.date_end || undefined,
        date_start: filter.date_start || undefined,
      });
    }
    if (parameter.data_type === "enum" && filter.option_id) {
      payload.push({
        parameter_type_id: filter.parameter_type_id,
        option_id: filter.option_id,
      });
    }
  }
  return payload.length > 0 ? JSON.stringify(payload) : undefined;
}

function optionalNumber(value: string): { ok: true; value: number | null } | { ok: false } {
  if (!value.trim()) return { ok: true, value: null };
  const number = Number(value);
  return Number.isFinite(number) ? { ok: true, value: number } : { ok: false };
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
