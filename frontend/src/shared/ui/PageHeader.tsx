import { type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { findRoute } from "../../app/modules";
import { useIsMobile } from "../lib/useIsMobile";

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
  const isMobile = useIsMobile();
  const location = useLocation();
  // The phone shell already names the screen in its bar. Repeating it directly
  // underneath wastes the top of a small screen and reads like a mistake — but
  // a detail page's title is the record's name, which the bar does not have, so
  // this compares rather than hides unconditionally. The heading stays in the
  // accessibility tree either way.
  const echoesTitleBar = isMobile && findRoute(location.pathname)?.title === title;

  return (
    <header className="page-header">
      <div>
        {kicker ? <p className="page-kicker">{kicker}</p> : null}
        <h1 className={echoesTitleBar ? "page-title sr-only" : "page-title"}>{title}</h1>
        {description ? <p className="page-description">{description}</p> : null}
      </div>
      {actions ? <div className="toolbar-group">{actions}</div> : null}
    </header>
  );
}
