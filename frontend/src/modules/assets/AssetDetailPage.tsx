import { useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Pencil, Plus } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { formatDate } from "../../shared/lib/date";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { EmptyState } from "../../shared/ui/EmptyState";
import { PageHeader } from "../../shared/ui/PageHeader";
import {
  type AssetCategory,
  type Unit,
  useAssetCategories,
  useAssetParameters,
  useLocations,
  useUnits,
} from "../admin/api";
import { AttachmentSection } from "../attachments/AttachmentPanel";
import { canManageLaboratoryAssets } from "../auth/permissions";
import {
  type Asset,
  type AssetInventoryItem,
  type AssetParameterValue,
  type AssetTrackingMode,
  assetQueryKeys,
  useAsset,
  useCreateAsset,
  useUpdateAsset,
} from "./api";
import { AssetEditor } from "./AssetsPage";
import { useCreateInventoryItems, useUpdateInventoryItem } from "../inventory/api";
import { InventoryEditor } from "../inventory/InventoryPage";

const EMPTY_CATEGORIES: AssetCategory[] = [];
const EMPTY_UNITS: Unit[] = [];

export function AssetDetailPage() {
  const { currentUser } = useAuth();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { assetId = "" } = useParams();
  const [editing, setEditing] = useState(false);
  const [creatingInventory, setCreatingInventory] = useState(false);
  const assetQuery = useAsset({ assetId, includeParameters: true });
  const asset = assetQuery.data;
  const canManage = canManageLaboratoryAssets(currentUser, asset?.laboratory_id);
  const categoriesQuery = useAssetCategories({
    enabled: Boolean(asset?.laboratory_id),
    laboratoryId: asset?.laboratory_id ?? "",
  });
  const locationsQuery = useLocations({
    enabled: Boolean(asset?.laboratory_id),
    laboratoryId: asset?.laboratory_id ?? "",
  });
  const parametersQuery = useAssetParameters({
    enabled: Boolean(asset?.laboratory_id),
    laboratoryId: asset?.laboratory_id ?? "",
  });
  const unitsQuery = useUnits();
  const createAsset = useCreateAsset();
  const updateAsset = useUpdateAsset();
  const createInventoryItems = useCreateInventoryItems();
  const updateInventoryItem = useUpdateInventoryItem();
  const categories = categoriesQuery.data ?? EMPTY_CATEGORIES;
  const units = unitsQuery.data ?? EMPTY_UNITS;
  const categoryById = new Map(categories.map((category) => [category.category_id, category]));
  const unitsById = new Map(units.map((unit) => [unit.unit_id, unit]));
  const locationsById = new Map(
    (locationsQuery.data ?? []).map((location) => [location.location_id, location]),
  );

  if (assetQuery.isLoading) {
    return (
      <main className="page">
        <PageHeader kicker="资产" title="资产详情" />
        <section className="panel">
          <div className="panel-body">
            <div className="skeleton" style={{ height: 260 }} />
          </div>
        </section>
      </main>
    );
  }

  if (!asset) {
    return (
      <main className="page">
        <PageHeader
          kicker="资产"
          title="资产详情"
          actions={
            <Button onClick={() => navigate("/assets")}>
              <ArrowLeft size={15} />
              返回资产
            </Button>
          }
        />
        <section className="panel">
          <EmptyState description="该资产不存在或当前账号没有访问权限。" title="未找到资产" />
        </section>
      </main>
    );
  }

  const inventoryColumns: DataTableColumn<AssetInventoryItem>[] = [
    {
      header: "库存项",
      key: "item",
      render: (item) => (
        <span className="asset-name-cell">
          <strong>{item.serial_number ?? item.batch_number ?? item.inventory_item_id}</strong>
          <span>{item.batch_number ? `批号 ${item.batch_number}` : "库存项"}</span>
        </span>
      ),
    },
    {
      header: "数量",
      key: "quantity",
      render: (item) => {
        const unit = unitsById.get(item.quantity_unit_id);
        return `${formatNumber(item.quantity_on_hand)}${unit ? ` ${unit.symbol}` : ""}`;
      },
    },
    {
      header: "已分配",
      key: "allocated",
      render: (item) => {
        const unit = unitsById.get(item.quantity_unit_id);
        return `${formatNumber(item.quantity_allocated)}${unit ? ` ${unit.symbol}` : ""}`;
      },
    },
    {
      header: "状态",
      key: "status",
      render: (item) => <Badge tone={inventoryStatusTone(item.status)}>{inventoryStatusLabel(item.status)}</Badge>,
    },
    {
      header: "位置",
      key: "location",
      render: (item) =>
        locationLabel(item.location_id, locationsById),
    },
    { header: "更新时间", key: "updated", render: (item) => formatDate(item.updated_at) },
  ];

  const parameterColumns: DataTableColumn<AssetParameterValue>[] = [
    {
      header: "参数",
      key: "parameter",
      render: (item) => (
        <span className="asset-name-cell">
          <strong>{item.name}</strong>
          <span>{item.code}</span>
        </span>
      ),
    },
    {
      header: "类型",
      key: "type",
      render: (item) => <Badge>{parameterTypeLabel(item.data_type)}</Badge>,
    },
    {
      header: "值",
      key: "value",
      render: (item) => formatParameterValue(item, unitsById),
    },
    { header: "更新时间", key: "updated", render: (item) => formatDate(item.updated_at) },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="资产"
        title={asset.name}
        description={asset.model ?? asset.manufacturer ?? undefined}
        actions={
          <>
            <Button onClick={() => navigate("/assets")}>
              <ArrowLeft size={15} />
              返回资产
            </Button>
            <Button disabled={!canManage} onClick={() => setEditing(true)} variant="primary">
              <Pencil size={15} />
              编辑资产/参数
            </Button>
            <Button
              disabled={!canManage}
              onClick={() => setCreatingInventory(true)}
              variant="primary"
            >
              <Plus size={15} />
              添加库存项
            </Button>
          </>
        }
      />

      <DetailCategoryPath asset={asset} categories={categories} />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">基本信息</h2>
            <p className="panel-description">资产标识、分类、管理模式和库存汇总。</p>
          </div>
        </div>
        <div className="panel-body">
          <dl className="asset-detail-grid">
            <DetailItem label="资产 ID" value={asset.asset_id} />
            <DetailItem label="分类" value={categoryLabel(asset.category_id, categoryById)} />
            <DetailItem label="管理模式" value={trackingModeLabel(asset.tracking_mode)} />
            <DetailItem label="厂商" value={asset.manufacturer ?? "未填写"} />
            <DetailItem label="型号" value={asset.model ?? "未填写"} />
            <DetailItem
              label="默认单位"
              value={unitsById.get(asset.default_unit_id)?.name ?? "未知单位"}
            />
            <DetailItem
              label="库存"
              value={`${asset.inventory_summary.item_count} 项 · ${formatNumber(
                asset.inventory_summary.quantity_on_hand,
              )}`}
            />
            <DetailItem
              label="已分配"
              value={formatNumber(asset.inventory_summary.quantity_allocated)}
            />
            <DetailItem label="创建时间" value={formatDate(asset.created_at)} />
            <DetailItem label="更新时间" value={formatDate(asset.updated_at)} />
            <DetailItem label="公开备注" value={asset.public_notes ?? "未填写"} />
            <DetailItem label="内部备注" value={asset.internal_notes ?? "未填写"} />
          </dl>
        </div>
      </section>

      <AttachmentSection
        canManage={canManage}
        laboratoryId={asset.laboratory_id}
        target={{ id: asset.asset_id, type: "asset" }}
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">库存项</h2>
            <p className="panel-description">{asset.inventory_items?.length ?? 0} 个库存项</p>
          </div>
        </div>
        <DataTable
          columns={inventoryColumns}
          emptyDescription="该资产还没有库存项。"
          getRowKey={(item) => item.inventory_item_id}
          items={asset.inventory_items ?? []}
          loading={locationsQuery.isLoading || unitsQuery.isLoading}
          onRowClick={(item) => navigate(`/inventory/${item.inventory_item_id}`)}
        />
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">参数信息</h2>
            <p className="panel-description">{asset.parameters?.length ?? 0} 个参数值</p>
          </div>
        </div>
        <DataTable
          columns={parameterColumns}
          emptyDescription="该资产还没有参数值。"
          getRowKey={(item) => item.value_id}
          items={asset.parameters ?? []}
          loading={unitsQuery.isLoading}
        />
      </section>

      <AssetEditor
        asset={asset}
        categories={categories}
        createAsset={createAsset}
        laboratoryId={asset.laboratory_id}
        open={editing}
        parameters={parametersQuery.data ?? []}
        units={units}
        updateAsset={updateAsset}
        onClose={() => setEditing(false)}
        onSaved={() => {
          setEditing(false);
          queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
        }}
      />
      <InventoryEditor
        categories={categories}
        createInventoryItems={createInventoryItems}
        editor={creatingInventory ? { asset, mode: "create" } : null}
        laboratoryId={asset.laboratory_id}
        locations={locationsQuery.data ?? []}
        open={creatingInventory}
        units={units}
        updateInventoryItem={updateInventoryItem}
        onClose={() => setCreatingInventory(false)}
        onSaved={() => {
          setCreatingInventory(false);
          queryClient.invalidateQueries({ queryKey: assetQueryKeys.root(apiBaseUrl) });
        }}
      />
    </main>
  );
}

function DetailCategoryPath({
  asset,
  categories,
}: {
  asset: Asset;
  categories: AssetCategory[];
}) {
  const breadcrumbs = buildCategoryBreadcrumbs(asset.category_id, categories);
  return (
    <nav className="asset-category-path" aria-label="资产分类路径">
      {breadcrumbs.map((breadcrumb, index) => (
        <span className="asset-category-path-item" key={breadcrumb.categoryId || "all"}>
          {index > 0 ? <span className="asset-category-path-separator">/</span> : null}
          <Link to={breadcrumb.categoryId ? `/assets?category_id=${breadcrumb.categoryId}` : "/assets"}>
            {breadcrumb.label}
          </Link>
        </span>
      ))}
    </nav>
  );
}

function DetailItem({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="asset-detail-item">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function buildCategoryBreadcrumbs(categoryId: string | null, categories: AssetCategory[]) {
  const selected = categories.find((category) => category.category_id === categoryId);
  if (!selected) {
    return [{ categoryId: "", label: "全部资产" }];
  }

  const breadcrumbs = [{ categoryId: "", label: "全部资产" }];
  const byId = new Map(categories.map((category) => [category.category_id, category]));
  const chain: AssetCategory[] = [];
  let current: AssetCategory | undefined = selected;
  const seen = new Set<string>();
  while (current && !seen.has(current.category_id)) {
    seen.add(current.category_id);
    chain.unshift(current);
    current = current.parent_category_id ? byId.get(current.parent_category_id) : undefined;
  }
  for (const category of chain) {
    breadcrumbs.push({ categoryId: category.category_id, label: category.name });
  }
  return breadcrumbs;
}

function categoryLabel(categoryId: string | null, categoryById: Map<string, AssetCategory>) {
  if (!categoryId) {
    return "未分类";
  }
  return categoryNamePath(categoryId, categoryById) ?? "未知分类";
}

function categoryNamePath(categoryId: string, categoryById: Map<string, AssetCategory>) {
  const names: string[] = [];
  let current = categoryById.get(categoryId);
  const seen = new Set<string>();
  while (current && !seen.has(current.category_id)) {
    seen.add(current.category_id);
    names.unshift(current.name);
    current = current.parent_category_id
      ? categoryById.get(current.parent_category_id)
      : undefined;
  }
  return names.length > 0 ? names.join(" / ") : null;
}

function locationLabel(locationId: string | null, locationById: Map<string, { location_id: string; name: string; parent_location_id: string | null }>) {
  if (!locationId) {
    return "未设置";
  }
  const names: string[] = [];
  let current = locationById.get(locationId);
  const seen = new Set<string>();
  while (current && !seen.has(current.location_id)) {
    seen.add(current.location_id);
    names.unshift(current.name);
    current = current.parent_location_id ? locationById.get(current.parent_location_id) : undefined;
  }
  return names.length > 0 ? names.join(" / ") : "未知位置";
}

function trackingModeLabel(mode: AssetTrackingMode) {
  return mode === "serialized" ? "序列号管理" : "数量管理";
}

function inventoryStatusLabel(status: string) {
  const labels: Record<string, string> = {
    available: "可用",
    checked_out: "借出",
    maintenance: "维护中",
    reserved: "预留",
    retired: "退役",
  };
  return labels[status] ?? status;
}

function inventoryStatusTone(status: string) {
  if (status === "available") return "success" as const;
  if (status === "maintenance" || status === "reserved") return "warning" as const;
  if (status === "retired") return "danger" as const;
  return "default" as const;
}

function parameterTypeLabel(type: AssetParameterValue["data_type"]) {
  const labels: Record<AssetParameterValue["data_type"], string> = {
    boolean: "布尔",
    date: "日期",
    enum: "枚举",
    number: "数值",
    range: "范围",
    text: "文本",
  };
  return labels[type];
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

function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: 4,
  }).format(value);
}
