import { RefreshCw } from "lucide-react";
import { useMemo } from "react";
import { useAuth } from "../../app/auth-context";
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
import { canRequestRemoteBorrow } from "../auth/permissions";
import { federationTrustLabel } from "../federation/api";
import {
  type LaboratoryDataScope,
  localLaboratoryScope,
  remoteLaboratoryScope,
} from "../federation/scope";
import {
  type ScopedBorrowRequest,
  useCancelBorrowRequest,
  useMyBorrowRequestsAcrossScopes,
} from "./api";
import { borrowRequestLabel, borrowRequestTone } from "./format";

export function MyBorrowRequestsPage() {
  const { currentUser } = useAuth();
  const { federationTrusts, selectedLaboratoryName } = useLaboratorySelection();
  const toast = useToast();
  const cancelBorrowRequest = useCancelBorrowRequest();
  const ownLaboratoryId = currentUser.laboratory?.laboratory_id ?? "";

  // A request lives only on the instance that owns the item, so this page asks
  // each laboratory the user can reach rather than one aggregate endpoint. A
  // guest is deliberately left with only their own laboratory: the federation
  // proxy refuses them, so asking a partner would only produce a 403.
  const scopes = useMemo<{ name: string; scope: LaboratoryDataScope }[]>(() => {
    const local = ownLaboratoryId
      ? [{ name: selectedLaboratoryName || "本实验室", scope: localLaboratoryScope(ownLaboratoryId) }]
      : [];
    if (!canRequestRemoteBorrow(currentUser)) {
      return local;
    }
    const remote = federationTrusts
      .filter((trust) => trust.status === "active")
      .map((trust) => ({
        name: federationTrustLabel(trust),
        scope: remoteLaboratoryScope(trust.remote_node_id, trust.remote_laboratory_id),
      }));
    return [...local, ...remote];
  }, [currentUser, federationTrusts, ownLaboratoryId, selectedLaboratoryName]);

  const { failureCount, isFetching, isLoading, refetch, requests } =
    useMyBorrowRequestsAcrossScopes(scopes);

  const columns: DataTableColumn<ScopedBorrowRequest>[] = useMemo(
    () => [
      { header: "申请时间", key: "created_at", render: (item) => formatDate(item.created_at) },
      {
        header: "实验室",
        key: "laboratory",
        render: (item) => (
          <span className="asset-name-cell">
            <strong>{item.laboratoryName}</strong>
            <span>{item.scope.kind === "remote" ? "远程实验室" : "本地实验室"}</span>
          </span>
        ),
      },
      {
        header: "资产",
        key: "asset",
        render: (item) => (
          <span className="asset-name-cell">
            <strong>{item.asset_name}</strong>
            <span>{item.asset_model ?? "无型号"}</span>
          </span>
        ),
      },
      { header: "申请备注", key: "note", render: (item) => item.request_note ?? "无" },
      {
        header: "状态",
        key: "status",
        render: (item) => (
          <Badge tone={borrowRequestTone(item.status)}>{borrowRequestLabel(item.status)}</Badge>
        ),
      },
      { header: "审批意见", key: "decision", render: (item) => item.decision_note ?? "无" },
      {
        align: "right",
        header: "操作",
        key: "actions",
        render: (item) =>
          item.status === "pending" ? (
            <ConfirmDialog
              confirmLabel="撤销"
              description={`撤销对「${item.asset_name}」的借用申请后，该库存项将重新开放申请。`}
              title="撤销借用申请"
              trigger={<Button variant="danger">撤销</Button>}
              onConfirm={() => handleCancel(item)}
            />
          ) : null,
      },
    ],
    [cancelBorrowRequest],
  );

  function handleCancel(request: ScopedBorrowRequest) {
    cancelBorrowRequest.mutate(
      { borrowRequestId: request.borrow_request_id, scope: request.scope },
      {
        onError: (error) =>
          toast.error({ title: "撤销失败", description: toErrorMessage(error) }),
        onSuccess: () => toast.success({ title: "已撤销借用申请" }),
      },
    );
  }

  if (scopes.length === 0) {
    return (
      <main className="page">
        <PageHeader kicker="借用" title="我的借用" />
        <section className="panel">
          <EmptyState description="当前账号没有归属实验室。" title="未选择实验室" />
        </section>
      </main>
    );
  }

  return (
    <main className="page">
      <PageHeader
        kicker="借用"
        title="我的借用"
        description="查看你在本实验室和各远程实验室发起的借用申请，并可撤销尚未审批的申请。"
        actions={
          <Button disabled={isFetching} onClick={() => refetch()}>
            <RefreshCw size={15} />
            刷新
          </Button>
        }
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">我发起的申请</h2>
            <p className="panel-description">
              共 {requests.length} 条申请，来自 {scopes.length} 个实验室。
              {failureCount > 0 ? ` 有 ${failureCount} 个实验室暂时无法访问。` : ""}
            </p>
          </div>
        </div>
        <DataTable
          columns={columns}
          emptyDescription={isLoading ? "申请加载中。" : "你还没有发起过借用申请。"}
          getRowKey={(item) => item.borrow_request_id}
          items={requests}
          loading={isLoading}
        />
      </section>
    </main>
  );
}
