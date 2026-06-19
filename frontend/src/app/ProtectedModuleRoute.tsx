import { ShieldAlert } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "../shared/ui/Button";
import { PageHeader } from "../shared/ui/PageHeader";
import { useAuth } from "./auth-context";
import { type ModuleRoute } from "./modules";

export function ProtectedModuleRoute({ route }: { route: ModuleRoute }) {
  const { currentUser } = useAuth();

  if (route.canAccess && !route.canAccess(currentUser)) {
    return (
      <main className="page">
        <PageHeader
          kicker="权限"
          title="无法访问"
          description="当前账号没有访问该页面的权限。"
          actions={
            <Button asChild>
              <Link to="/dashboard">返回概览</Link>
            </Button>
          }
        />
        <section className="panel">
          <div className="empty-state">
            <ShieldAlert size={24} aria-hidden="true" />
            <div>
              <h3>权限不足</h3>
              <p>请切换账号，或联系管理员调整权限。</p>
            </div>
          </div>
        </section>
      </main>
    );
  }

  return route.element;
}
