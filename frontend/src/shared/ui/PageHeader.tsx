import { type ReactNode } from "react";

export function PageHeader({
  actions,
  description,
  kicker,
  title,
}: {
  actions?: ReactNode;
  description?: string;
  kicker?: string;
  title: string;
}) {
  return (
    <header className="page-header">
      <div>
        {kicker ? <p className="page-kicker">{kicker}</p> : null}
        <h1 className="page-title">{title}</h1>
        {description ? <p className="page-description">{description}</p> : null}
      </div>
      {actions ? <div className="toolbar-group">{actions}</div> : null}
    </header>
  );
}
