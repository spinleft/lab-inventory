import { type ReactNode } from "react";
import { EmptyState } from "./EmptyState";

export type DataTableColumn<T> = {
  align?: "left" | "right";
  header: ReactNode;
  key: string;
  render: (item: T) => ReactNode;
};

type DataTableProps<T> = {
  columns: DataTableColumn<T>[];
  emptyDescription?: string;
  emptyTitle?: string;
  getRowKey: (item: T) => string;
  items: T[];
  loading?: boolean;
  onRowClick?: (item: T) => void;
  /** Called with the full next selection whenever a checkbox changes. */
  onSelectionChange?: (keys: string[]) => void;
  /** Adds a leading checkbox column. Requires `onSelectionChange`. */
  selectable?: boolean;
  /** Row keys currently selected. */
  selectedKeys?: string[];
};

export function DataTable<T>({
  columns,
  emptyDescription = "没有可显示的数据。",
  emptyTitle = "暂无数据",
  getRowKey,
  items,
  loading = false,
  onRowClick,
  onSelectionChange,
  selectable = false,
  selectedKeys,
}: DataTableProps<T>) {
  const selection = new Set(selectedKeys ?? []);
  const pageKeys = items.map(getRowKey);
  const allSelected = pageKeys.length > 0 && pageKeys.every((key) => selection.has(key));
  const someSelected = pageKeys.some((key) => selection.has(key));

  function toggleRow(key: string, checked: boolean) {
    const next = new Set(selection);
    if (checked) {
      next.add(key);
    } else {
      next.delete(key);
    }
    onSelectionChange?.([...next]);
  }

  function toggleAll(checked: boolean) {
    const next = new Set(selection);
    for (const key of pageKeys) {
      if (checked) {
        next.add(key);
      } else {
        next.delete(key);
      }
    }
    onSelectionChange?.([...next]);
  }

  if (loading) {
    return (
      <div className="panel-body">
        <div className="skeleton" style={{ height: 180 }} />
      </div>
    );
  }

  if (items.length === 0) {
    return <EmptyState description={emptyDescription} title={emptyTitle} />;
  }

  return (
    <div className="table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            {selectable ? (
              <th className="data-table-select">
                <input
                  aria-label="全选本页"
                  checked={allSelected}
                  ref={(node) => {
                    if (node) {
                      node.indeterminate = someSelected && !allSelected;
                    }
                  }}
                  type="checkbox"
                  onChange={(event) => toggleAll(event.target.checked)}
                />
              </th>
            ) : null}
            {columns.map((column) => (
              <th
                key={column.key}
                style={{ textAlign: column.align === "right" ? "right" : "left" }}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {items.map((item) => (
            <tr
              className={onRowClick ? "asset-clickable-row" : undefined}
              key={getRowKey(item)}
              tabIndex={onRowClick ? 0 : undefined}
              onClick={onRowClick ? () => onRowClick(item) : undefined}
              onKeyDown={
                onRowClick
                  ? (event) => {
                      if (event.key === "Enter") onRowClick(item);
                    }
                  : undefined
              }
            >
              {selectable ? (
                // Clicking a row navigates, so the checkbox has to keep its
                // click to itself.
                <td
                  className="data-table-select"
                  onClick={(event) => event.stopPropagation()}
                >
                  <input
                    aria-label="选择此行"
                    checked={selection.has(getRowKey(item))}
                    type="checkbox"
                    onChange={(event) => toggleRow(getRowKey(item), event.target.checked)}
                  />
                </td>
              ) : null}
              {columns.map((column) => (
                <td
                  key={column.key}
                  style={{ textAlign: column.align === "right" ? "right" : "left" }}
                >
                  {column.render(item)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
