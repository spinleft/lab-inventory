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
};

export function DataTable<T>({
  columns,
  emptyDescription = "没有可显示的数据。",
  emptyTitle = "暂无数据",
  getRowKey,
  items,
  loading = false,
  onRowClick,
}: DataTableProps<T>) {
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
