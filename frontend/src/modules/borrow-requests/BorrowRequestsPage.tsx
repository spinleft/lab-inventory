import { RefreshCw } from "lucide-react";
import { type ReactNode, useMemo } from "react";
import { Link } from "react-router-dom";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { ConfirmDialog } from "../../shared/ui/ConfirmDialog";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { EmptyState } from "../../shared/ui/EmptyState";
import { PageHeader } from "../../shared/ui/PageHeader";
import { useToast } from "../../shared/ui/Toast";
import { type InventoryItem, useInventoryItems } from "../inventory/api";
import { inventoryStatusLabel, inventoryStatusTone } from "../inventory/format";
import { type BorrowRequest, useBorrowRequests, useResolveBorrowRequest } from "./api";

export function BorrowRequestsPage() {
  const { selectedDataScope, selectedLaboratoryId } = useLaboratorySelection();
  const toast = useToast();
  const resolveBorrowRequest = useResolveBorrowRequest();
  const pendingRequestsQuery = useBorrowRequests({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    status: "pending",
  });
  const borrowedItemsQuery = useInventoryItems({
    enabled: Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
    query: { limit: 200, offset: 0, status: "borrowed" },
    scope: selectedDataScope,
  });
  const pendingRequests = pendingRequestsQuery.data ?? [];
  const borrowedItems = borrowedItemsQuery.data?.items ?? [];

  const pendingColumns: DataTableColumn<BorrowRequest>[] = useMemo(
    () => [
      {
        header: "申请时间",
        key: "created_at",
        render: (item) => formatDate(item.created_at),
      },
      {
        header: "申请人",
        key: "requester",
        render: (item) => (
          <span className="asset-name-cell">
            <strong>{item.requester_username}</strong>
            <span>{item.requester_user_type}</span>
          </span>
        ),
      },
      {
        header: "库存项",
        key: "inventory",
        render: (item) => (
          <span className="asset-name-cell">
            <strong>{item.inventory_item_title}</strong>
            <span>{item.asset_name}</span>
          </span>
        ),
      },
      { header: "申请备注", key: "note", render: (item) => item.request_note ?? "无" },
      {
        header: "状态",
        key: "status",
        render: (item) => <Badge tone={borrowRequestTone(item.status)}>{borrowRequestLabel(item.status)}</Badge>,
      },
      {
        align: "right",
        header: "操作",
        key: "actions",
        render: (item) => (
          <span className="table-actions">
            <ConfirmDialog
              confirmLabel="批准"
              description={`批准该借用申请后，库存项「${item.inventory_item_title}」将标记为借出。`}
              title="批准借用申请"
              trigger={<Button variant="primary">批准</Button>}
              onConfirm={() => handleResolve(item, "approved")}
            />
            <ConfirmDialog
              confirmLabel="拒绝"
              description={`拒绝该借用申请「${item.inventory_item_title}」。`}
              title="拒绝借用申请"
              trigger={<Button variant="danger">拒绝</Button>}
              onConfirm={() => handleResolve(item, "rejected")}
            />
          </span>
        ),
      },
    ],
    [resolveBorrowRequest],
  );

  const borrowedColumns: DataTableColumn<InventoryItem>[] = useMemo(
    () => [
      {
        header: "库存项",
        key: "title",
        render: (item) => (
          <span className="asset-name-cell">
            <strong>{inventoryItemTitle(item)}</strong>
            <span>{item.asset.name}</span>
          </span>
        ),
      },
      { header: "状态", key: "status", render: (item) => <Badge tone={inventoryStatusTone(item.status)}>{inventoryStatusLabel(item.status)}</Badge> },
      {
        header: "位置",
        key: "location",
        render: (item) => item.location_id ?? "未设置",
      },
      { header: "更新时间", key: "updated_at", render: (item) => formatDate(item.updated_at) },
      {
        align: "right",
        header: "详情",
        key: "detail",
        render: (item) => (
          <Button asChild variant="ghost">
            <Link to={`/inventory/${item.inventory_item_id}`}>查看</Link>
          </Button>
        ),
      },
    ],
    [],
  );

  function handleResolve(request: BorrowRequest, decision: "approved" | "rejected") {
    if (!selectedLaboratoryId) {
      return;
    }
    resolveBorrowRequest.mutate(
      {
        borrowRequestId: request.borrow_request_id,
        laboratoryId: selectedLaboratoryId,
        payload: { decision },
      },
      {
        onError: (error) =>
          toast.error({ title: decision === "approved" ? "批准失败" : "拒绝失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          toast.success({ title: decision === "approved" ? "已批准借用申请" : "已拒绝借用申请" });
        },
      },
    );
  }

  if (!selectedLaboratoryId) {
    return (
      <main className="page">
        <PageHeader kicker="借用" title="借用管理" />
        <section className="panel">
          <EmptyState description="请先选择一个实验室。" title="未选择实验室" />
        </section>
      </main>
    );
  }

  return (
    <main className="page">
      <PageHeader
        kicker="借用"
        title="借用管理"
        description="查看本实验室待审批的借用申请，以及当前已借出的库存项。"
        actions={
          <Button
            disabled={pendingRequestsQuery.isFetching || borrowedItemsQuery.isFetching}
            onClick={() => {
              pendingRequestsQuery.refetch();
              borrowedItemsQuery.refetch();
            }}
          >
            <RefreshCw size={15} />
            刷新
          </Button>
        }
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">待审批申请</h2>
            <p className="panel-description">当前共有 {pendingRequests.length} 条待处理申请。</p>
          </div>
        </div>
        <DataTable
          columns={pendingColumns}
          emptyDescription={pendingRequestsQuery.isLoading ? "申请加载中。" : "当前没有待审批的借用申请。"}
          getRowKey={(item) => item.borrow_request_id}
          items={pendingRequests}
          loading={pendingRequestsQuery.isLoading}
        />
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">已借出库存</h2>
            <p className="panel-description">当前共有 {borrowedItems.length} 个库存项处于借出状态。</p>
          </div>
        </div>
        <DataTable
          columns={borrowedColumns}
          emptyDescription={borrowedItemsQuery.isLoading ? "库存加载中。" : "当前没有借出的库存项。"}
          getRowKey={(item) => item.inventory_item_id}
          items={borrowedItems}
          loading={borrowedItemsQuery.isLoading}
        />
      </section>
    </main>
  );
}

function borrowRequestLabel(status: BorrowRequest["status"]) {
  if (status === "pending") return "待审批";
  if (status === "approved") return "已批准";
  return "已拒绝";
}

function borrowRequestTone(status: BorrowRequest["status"]) {
  if (status === "pending") return "warning" as const;
  if (status === "approved") return "success" as const;
  return "danger" as const;
}

function inventoryItemTitle(item: InventoryItem) {
  return item.serial_number ?? item.batch_number ?? item.inventory_item_id;
}