import { ArrowLeft, PackageSearch, Pencil } from "lucide-react";
import { type ReactNode, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useAuth } from "../../app/auth-context";
import { formatDate } from "../../shared/lib/date";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { EmptyState } from "../../shared/ui/EmptyState";
import { PageHeader } from "../../shared/ui/PageHeader";
import {
  type AssetCategory,
  type Location,
  type Unit,
  useAssetCategories,
  useAssetParameters,
  useLocations,
  useUnits,
} from "../admin/api";
import { AttachmentSection } from "../attachments/AttachmentPanel";
import { canManageLaboratoryAssets } from "../auth/permissions";
import { type AssetParameterValue, useAsset } from "../assets/api";
import {
  type InventoryItem,
  useCreateInventoryItems,
  useInventoryItem,
  useUpdateInventoryItem,
} from "./api";
import {
  categoryLabel,
  formatNumber,
  formatParameterValue,
  inventoryStatusLabel,
  inventoryStatusTone,
  locationLabel,
  parameterTypeLabel,
  trackingModeLabel,
  unitLabel,
} from "./format";
import { InventoryEditor } from "./InventoryPage";

const EMPTY_CATEGORIES: AssetCategory[] = [];
const EMPTY_LOCATIONS: Location[] = [];
const EMPTY_UNITS: Unit[] = [];

export function InventoryDetailPage() {
  const { currentUser } = useAuth();
  const navigate = useNavigate();
  const { inventoryItemId = "" } = useParams();
  const [editing, setEditing] = useState(false);
  const inventoryQuery = useInventoryItem({ inventoryItemId });
  const item = inventoryQuery.data;
  const canManage = canManageLaboratoryAssets(currentUser, item?.laboratory_id);
  const assetQuery = useAsset({
    assetId: item?.asset_id ?? "",
    enabled: Boolean(item?.asset_id),
    includeParameters: true,
  });
  const categoriesQuery = useAssetCategories({
    enabled: Boolean(item?.laboratory_id),
    laboratoryId: item?.laboratory_id ?? "",
  });
  const locationsQuery = useLocations({
    enabled: Boolean(item?.laboratory_id),
    laboratoryId: item?.laboratory_id ?? "",
  });
  const parametersQuery = useAssetParameters({
    enabled: Boolean(item?.laboratory_id),
    laboratoryId: item?.laboratory_id ?? "",
  });
  const unitsQuery = useUnits();
  const createInventoryItems = useCreateInventoryItems();
  const updateInventoryItem = useUpdateInventoryItem();
  const categories = categoriesQuery.data ?? EMPTY_CATEGORIES;
  const locations = locationsQuery.data ?? EMPTY_LOCATIONS;
  const units = unitsQuery.data ?? EMPTY_UNITS;
  const categoryById = useMemo(() => mapById(categories, "category_id"), [categories]);
  const locationById = useMemo(() => mapById(locations, "location_id"), [locations]);
  const unitsById = useMemo(() => mapById(units, "unit_id"), [units]);

  if (inventoryQuery.isLoading) {
    return (
      <main className="page">
        <PageHeader kicker="库存" title="库存详情" />
        <section className="panel">
          <div className="panel-body">
            <div className="skeleton" style={{ height: 260 }} />
          </div>
        </section>
      </main>
    );
  }

  if (!item) {
    return (
      <main className="page">
        <PageHeader
          kicker="库存"
          title="库存详情"
          actions={
            <Button onClick={() => navigate("/inventory")}>
              <ArrowLeft size={15} />
              返回库存
            </Button>
          }
        />
        <section className="panel">
          <EmptyState
            description="该库存项不存在，或当前账号没有访问权限。"
            title="未找到库存"
          />
        </section>
      </main>
    );
  }

  const asset = assetQuery.data;
  const parameterColumns: DataTableColumn<AssetParameterValue>[] = [
    {
      header: "参数",
      key: "parameter",
      render: (value) => (
        <span className="asset-name-cell">
          <strong>{value.name}</strong>
          <span>{value.code}</span>
        </span>
      ),
    },
    {
      header: "类型",
      key: "type",
      render: (value) => <Badge>{parameterTypeLabel(value.data_type)}</Badge>,
    },
    {
      header: "值",
      key: "value",
      render: (value) => formatParameterValue(value, unitsById),
    },
    { header: "更新时间", key: "updated", render: (value) => formatDate(value.updated_at) },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="库存"
        title={inventoryItemTitle(item)}
        description={`${item.asset.name} · ${trackingModeLabel(item.tracking_mode)}`}
        actions={
          <>
            <Button onClick={() => navigate("/inventory")}>
              <ArrowLeft size={15} />
              返回库存
            </Button>
            <Button asChild variant="primary">
              <Link to={`/assets/${item.asset_id}`}>
                <PackageSearch size={15} />
                查看资产
              </Link>
            </Button>
            <Button disabled={!canManage} onClick={() => setEditing(true)} variant="primary">
              <Pencil size={15} />
              编辑库存
            </Button>
          </>
        }
      />

      <DetailLocationPath item={item} locations={locations} />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">基本信息</h2>
            <p className="panel-description">库存项标识、状态、数量、位置和备注。</p>
          </div>
          <Badge tone={inventoryStatusTone(item.status)}>
            {inventoryStatusLabel(item.status)}
          </Badge>
        </div>
        <div className="panel-body">
          <dl className="asset-detail-grid">
            <DetailItem label="库存项 ID" value={item.inventory_item_id} />
            <DetailItem label="资产" value={<Link to={`/assets/${item.asset_id}`}>{item.asset.name}</Link>} />
            <DetailItem label="分类" value={categoryLabel(item.asset.category_id, categoryById)} />
            <DetailItem label="管理模式" value={trackingModeLabel(item.tracking_mode)} />
            <DetailItem label="序列号" value={item.serial_number ?? "未填写"} />
            <DetailItem label="批号" value={item.batch_number ?? "未填写"} />
            <DetailItem label="位置" value={locationLabel(item.location_id, locationById)} />
            <DetailItem
              label="库存数量"
              value={`${formatNumber(item.quantity_on_hand)} ${unitLabel(item.quantity_unit_id, unitsById)}`}
            />
            <DetailItem
              label="已分配"
              value={`${formatNumber(item.quantity_allocated)} ${unitLabel(item.quantity_unit_id, unitsById)}`}
            />
            <DetailItem label="公开备注" value={item.public_notes ?? "未填写"} />
            <DetailItem label="内部备注" value={item.internal_notes ?? "未填写"} />
            <DetailItem label="最近盘点" value={formatDate(item.last_stocktake_at)} />
            <DetailItem label="创建时间" value={formatDate(item.created_at)} />
            <DetailItem label="更新时间" value={formatDate(item.updated_at)} />
          </dl>
        </div>
      </section>

      <AttachmentSection
        canManage={canManage}
        laboratoryId={item.laboratory_id}
        target={{ id: item.inventory_item_id, type: "inventory-item" }}
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">关联资产</h2>
            <p className="panel-description">库存项所属资产的基础信息。</p>
          </div>
        </div>
        <div className="panel-body">
          <dl className="asset-detail-grid">
            <DetailItem label="资产名称" value={item.asset.name} />
            <DetailItem label="型号" value={item.asset.model ?? "未填写"} />
            <DetailItem label="厂商" value={item.asset.manufacturer ?? "未填写"} />
            <DetailItem
              label="默认单位"
              value={unitLabel(item.asset.default_unit_id, unitsById)}
            />
          </dl>
        </div>
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">参数信息</h2>
            <p className="panel-description">
              {asset?.parameters?.length ?? 0} 个资产参数
            </p>
          </div>
        </div>
        <DataTable
          columns={parameterColumns}
          emptyDescription={
            parametersQuery.isLoading ? "参数加载中。" : "关联资产还没有参数值。"
          }
          getRowKey={(value) => value.value_id}
          items={asset?.parameters ?? []}
          loading={assetQuery.isLoading || unitsQuery.isLoading}
        />
      </section>
      <InventoryEditor
        categories={categories}
        createInventoryItems={createInventoryItems}
        editor={editing ? { item, mode: "edit" } : null}
        laboratoryId={item.laboratory_id}
        locations={locations}
        open={editing}
        units={units}
        updateInventoryItem={updateInventoryItem}
        onClose={() => setEditing(false)}
        onSaved={() => setEditing(false)}
      />
    </main>
  );
}

function DetailLocationPath({
  item,
  locations,
}: {
  item: InventoryItem;
  locations: Location[];
}) {
  const breadcrumbs = buildLocationBreadcrumbs(item.location_id, locations);
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

function DetailItem({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="asset-detail-item">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function inventoryItemTitle(item: InventoryItem) {
  return item.serial_number ?? item.batch_number ?? item.inventory_item_id;
}

function buildLocationBreadcrumbs(locationId: string | null, locations: Location[]) {
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

function mapById<T extends Record<K, string>, K extends keyof T>(items: T[], key: K) {
  return new Map(items.map((item) => [item[key], item]));
}
