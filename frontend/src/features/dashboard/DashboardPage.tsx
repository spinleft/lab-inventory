import {
  AlertTriangle,
  ClipboardCheck,
  PackageMinus,
} from "lucide-react";
import {
  useBorrowRequestAlerts,
  useMaintenanceAlerts,
  useStockAlerts,
} from "./api";

export function DashboardPage() {
  const stockAlerts = useStockAlerts();
  const borrowAlerts = useBorrowRequestAlerts();
  const maintenanceAlerts = useMaintenanceAlerts();

  return (
    <>
      <div className="page-header">
        <div>
          <h1>概览</h1>
          <p>待审批、低库存和维护提醒会优先显示在这里。</p>
        </div>
      </div>

      <section className="metrics-grid" aria-label="提醒概览">
        <MetricCard
          icon={<PackageMinus size={22} />}
          label="低库存"
          value={stockAlerts.data?.length ?? 0}
          loading={stockAlerts.isLoading}
        />
        <MetricCard
          icon={<ClipboardCheck size={22} />}
          label="借用提醒"
          value={borrowAlerts.data?.length ?? 0}
          loading={borrowAlerts.isLoading}
        />
        <MetricCard
          icon={<AlertTriangle size={22} />}
          label="维护提醒"
          value={maintenanceAlerts.data?.length ?? 0}
          loading={maintenanceAlerts.isLoading}
        />
      </section>

      <section className="panel panel-pad">
        <h2 className="section-title">近期提醒</h2>
        <div className="reminder-list">
          {stockAlerts.data?.slice(0, 4).map((alert) => (
            <div className="reminder-item" key={alert.asset_id}>
              <span className="badge">低库存</span>
              <div>
                <strong>{alert.name}</strong>
                <p className="muted small">
                  {alert.laboratory_name} · 可用 {alert.quantity_available}
                </p>
              </div>
            </div>
          ))}
          {borrowAlerts.data?.slice(0, 4).map((alert) => (
            <div className="reminder-item" key={alert.borrow_request_id}>
              <span className="badge">借用</span>
              <div>
                <strong>{alert.asset_name}</strong>
                <p className="muted small">
                  {alert.requester_laboratory_name} 到 {alert.owner_laboratory_name}
                </p>
              </div>
            </div>
          ))}
          {maintenanceAlerts.data?.slice(0, 4).map((alert) => (
            <div className="reminder-item" key={alert.maintenance_schedule_id}>
              <span className="badge">维护</span>
              <div>
                <strong>{alert.asset_name}</strong>
                <p className="muted small">
                  {alert.schedule_name} · {formatDate(alert.next_maintenance_at)}
                </p>
              </div>
            </div>
          ))}
          {noAlerts(
            stockAlerts.data?.length,
            borrowAlerts.data?.length,
            maintenanceAlerts.data?.length,
          ) ? (
            <p className="muted">暂无提醒。</p>
          ) : null}
        </div>
      </section>
    </>
  );
}

function MetricCard({
  icon,
  label,
  value,
  loading,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  loading: boolean;
}) {
  return (
    <div className="metric-card">
      <div className="metric-icon">{icon}</div>
      <div>
        <div className="metric-label">{label}</div>
        <div className="metric-value">{loading ? "..." : value}</div>
      </div>
    </div>
  );
}

function noAlerts(...counts: Array<number | undefined>) {
  return counts.every((count) => count === 0);
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
