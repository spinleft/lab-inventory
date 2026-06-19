import { Building2, ScrollText, Users } from "lucide-react";
import { Link } from "react-router-dom";
import { canAccessAdmin, canAccessAuditLogs } from "../auth/permissions";
import { useAuth } from "../../app/auth-context";
import { Button } from "../../shared/ui/Button";
import { PageHeader } from "../../shared/ui/PageHeader";

export function DashboardPage() {
  const { currentUser } = useAuth();

  return (
    <main className="page">
      <PageHeader title="概览" />

      <section className="stats-grid" aria-label="系统状态">
        <div className="stat">
          <p className="stat-label">当前用户</p>
          <p className="stat-value">{currentUser.username}</p>
        </div>
        <div className="stat">
          <p className="stat-label">数据范围</p>
          <p className="stat-value">{currentUser.laboratory?.name ?? "全部"}</p>
        </div>
        <div className="stat">
          <p className="stat-label">状态</p>
          <p className="stat-value">在线</p>
        </div>
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">常用入口</h2>
            <p className="panel-description">根据当前账号权限显示可用管理入口。</p>
          </div>
        </div>
        <div className="panel-body">
          <div className="toolbar-group">
            {canAccessAdmin(currentUser) ? (
              <>
                <Button asChild>
                  <Link to="/admin/laboratories">
                    <Building2 size={15} />
                    实验室
                  </Link>
                </Button>
                <Button asChild>
                  <Link to="/admin/users">
                    <Users size={15} />
                    用户
                  </Link>
                </Button>
              </>
            ) : null}
            {canAccessAuditLogs(currentUser) ? (
              <Button asChild>
                <Link to="/audit-logs">
                  <ScrollText size={15} />
                  审计日志
                </Link>
              </Button>
            ) : null}
          </div>
        </div>
      </section>
    </main>
  );
}
