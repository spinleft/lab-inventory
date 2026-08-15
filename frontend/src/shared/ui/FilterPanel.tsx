import * as DialogPrimitive from "@radix-ui/react-dialog";
import { SlidersHorizontal, X } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useIsMobile } from "../lib/useIsMobile";
import { Button } from "./Button";

type FilterPanelProps = {
  /** How many filters are currently narrowing the list, for the badge. */
  activeCount?: number;
  children: ReactNode;
  description?: string;
  title?: string;
};

/**
 * A list's filters, in the shape the viewport can afford.
 *
 * On a desktop they are a panel above the table, always visible. On a phone
 * that panel is the whole first screen — the user scrolls past a form they did
 * not ask for to reach the data — so it collapses to a button that opens a
 * sheet, with a badge so an active filter is never invisible.
 */
export function FilterPanel({
  activeCount = 0,
  children,
  description,
  title = "筛选",
}: FilterPanelProps) {
  const isMobile = useIsMobile();
  const [open, setOpen] = useState(false);

  if (!isMobile) {
    return (
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">{title}</h2>
            {description ? <p className="panel-description">{description}</p> : null}
          </div>
        </div>
        <div className="panel-body">{children}</div>
      </section>
    );
  }

  return (
    <DialogPrimitive.Root open={open} onOpenChange={setOpen}>
      <DialogPrimitive.Trigger asChild>
        <Button className="filter-trigger">
          <SlidersHorizontal size={15} />
          {title}
          {activeCount > 0 ? <span className="filter-badge">{activeCount}</span> : null}
        </Button>
      </DialogPrimitive.Trigger>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="dialog-overlay" />
        <DialogPrimitive.Content className="dialog-content sheet-content">
          <div className="dialog-header">
            <div>
              <DialogPrimitive.Title className="dialog-title">{title}</DialogPrimitive.Title>
              {description ? (
                <DialogPrimitive.Description className="dialog-description">
                  {description}
                </DialogPrimitive.Description>
              ) : null}
            </div>
            <DialogPrimitive.Close asChild>
              <Button size="icon" variant="ghost" aria-label="关闭">
                <X size={16} />
              </Button>
            </DialogPrimitive.Close>
          </div>
          {/* Applying the filters should return the user to the results they
              just changed; `submit` bubbles, so the form inside needs no
              knowledge of the sheet it happens to be sitting in. */}
          <div className="dialog-body" onSubmitCapture={() => setOpen(false)}>
            {children}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
