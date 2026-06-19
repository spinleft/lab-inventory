import { ChevronLeft, ChevronRight, Filter, RotateCcw } from "lucide-react";
import { type FormEvent, useMemo, useState } from "react";
import { formatDate, toIsoDateTime } from "../../shared/lib/date";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { Dialog } from "../../shared/ui/Dialog";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { Select } from "../../shared/ui/Select";
import { type AuditLog, type AuditLogQuery, useAuditLogs } from "./api";

type AuditFilterForm = {
  action: string;
  actor_user_id: string;
  created_from: string;
  created_to: string;
  resource_id: string;
  resource_type: string;
};

const PAGE_SIZE = 50;

export function AuditLogsPage() {
  const [offset, setOffset] = useState(0);
  const [filters, setFilters] = useState<AuditFilterForm>(emptyFilters());
  const [draftFilters, setDraftFilters] = useState<AuditFilterForm>(emptyFilters());
  const [detailLog, setDetailLog] = useState<AuditLog | null>(null);
  const query = useMemo<AuditLogQuery>(
    () => ({
      action: optional(filters.action),
      actor_user_id: optional(filters.actor_user_id),
      created_from: toIsoDateTime(filters.created_from) ?? undefined,
      created_to: toIsoDateTime(filters.created_to) ?? undefined,
      limit: PAGE_SIZE,
      offset,
      resource_id: optional(filters.resource_id),
      resource_type: optional(filters.resource_type),
    }),
    [filters, offset],
  );
  const auditLogsQuery = useAuditLogs(query);
  const response = auditLogsQuery.data;
  const total = response?.total ?? 0;
  const items = response?.items ?? [];
  const page = Math.floor(offset / PAGE_SIZE) + 1;
  const maxPage = Math.max(1, Math.ceil(total / PAGE_SIZE));

  function submitFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setOffset(0);
    setFilters(draftFilters);
  }

  function resetFilters() {
    const next = emptyFilters();
    setDraftFilters(next);
    setFilters(next);
    setOffset(0);
  }

  const columns: DataTableColumn<AuditLog>[] = [
    { header: "时间", key: "created", render: (item) => formatDate(item.created_at) },
    { header: "操作者", key: "actor", render: (item) => item.actor_username ?? "系统" },
    {
      header: "动作",
      key: "action",
      render: (item) => <Badge tone={actionTone(item.action)}>{item.action}</Badge>,
    },
    { header: "资源", key: "resource", render: (item) => item.resource_type },
    {
      header: "资源 ID",
      key: "resourceId",
      render: (item) => item.resource_id ?? "无",
    },
    {
      align: "right",
      header: "详情",
      key: "detail",
      render: (item) => (
        <Button variant="ghost" onClick={() => setDetailLog(item)}>
          查看
        </Button>
      ),
    },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="审计"
        title="审计日志"
        description="查看关键管理操作的追踪记录。仅根用户和超级管理员可访问。"
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">筛选</h2>
            <p className="panel-description">按操作者、资源、动作和时间范围缩小日志范围。</p>
          </div>
        </div>
        <div className="panel-body">
          <form className="form-grid" onSubmit={submitFilters}>
            <div className="form-grid form-grid-2">
              <FormField label="动作" htmlFor="audit-action">
                <Select
                  id="audit-action"
                  label="动作"
                  options={[
                    { label: "全部", value: "all" },
                    { label: "create", value: "create" },
                    { label: "update", value: "update" },
                    { label: "delete", value: "delete" },
                  ]}
                  value={draftFilters.action || "all"}
                  onValueChange={(value) =>
                    setDraftFilters((current) => ({
                      ...current,
                      action: value === "all" ? "" : value,
                    }))
                  }
                />
              </FormField>
              <FormField label="资源类型" htmlFor="audit-resource-type">
                <input
                  className="input"
                  id="audit-resource-type"
                  placeholder="user / laboratory"
                  value={draftFilters.resource_type}
                  onChange={(event) =>
                    setDraftFilters((current) => ({
                      ...current,
                      resource_type: event.target.value,
                    }))
                  }
                />
              </FormField>
              <FormField label="操作者 ID" htmlFor="audit-actor">
                <input
                  className="input"
                  id="audit-actor"
                  value={draftFilters.actor_user_id}
                  onChange={(event) =>
                    setDraftFilters((current) => ({
                      ...current,
                      actor_user_id: event.target.value,
                    }))
                  }
                />
              </FormField>
              <FormField label="资源 ID" htmlFor="audit-resource-id">
                <input
                  className="input"
                  id="audit-resource-id"
                  value={draftFilters.resource_id}
                  onChange={(event) =>
                    setDraftFilters((current) => ({
                      ...current,
                      resource_id: event.target.value,
                    }))
                  }
                />
              </FormField>
              <FormField label="开始时间" htmlFor="audit-from">
                <input
                  className="input"
                  id="audit-from"
                  type="datetime-local"
                  value={draftFilters.created_from}
                  onChange={(event) =>
                    setDraftFilters((current) => ({
                      ...current,
                      created_from: event.target.value,
                    }))
                  }
                />
              </FormField>
              <FormField label="结束时间" htmlFor="audit-to">
                <input
                  className="input"
                  id="audit-to"
                  type="datetime-local"
                  value={draftFilters.created_to}
                  onChange={(event) =>
                    setDraftFilters((current) => ({
                      ...current,
                      created_to: event.target.value,
                    }))
                  }
                />
              </FormField>
            </div>
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
        <div className="panel-header">
          <div>
            <h2 className="panel-title">日志列表</h2>
            <p className="panel-description">
              第 {page} / {maxPage} 页，共 {total} 条
            </p>
          </div>
          <div className="toolbar-group">
            <Button
              disabled={offset <= 0 || auditLogsQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="上一页"
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              <ChevronLeft size={16} />
            </Button>
            <Button
              disabled={offset + PAGE_SIZE >= total || auditLogsQuery.isFetching}
              size="icon"
              variant="ghost"
              aria-label="下一页"
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              <ChevronRight size={16} />
            </Button>
          </div>
        </div>
        <DataTable
          columns={columns}
          emptyDescription="当前筛选条件下没有审计日志。"
          getRowKey={(item) => item.audit_log_id}
          items={items}
          loading={auditLogsQuery.isLoading}
        />
      </section>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setDetailLog(null);
        }}
        open={detailLog !== null}
        title="日志详情"
      >
        <pre className="json-view">
          {detailLog ? JSON.stringify(detailLog.details, null, 2) : ""}
        </pre>
      </Dialog>
    </main>
  );
}

function actionTone(action: string) {
  if (action === "create") {
    return "success" as const;
  }
  if (action === "update") {
    return "warning" as const;
  }
  if (action === "delete") {
    return "danger" as const;
  }
  return "default" as const;
}

function emptyFilters(): AuditFilterForm {
  return {
    action: "",
    actor_user_id: "",
    created_from: "",
    created_to: "",
    resource_id: "",
    resource_type: "",
  };
}

function optional(value: string) {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}
