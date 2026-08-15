import { type ReactNode } from "react";
import { useIsMobile } from "../lib/useIsMobile";
import { EmptyState } from "./EmptyState";

export type DataTableColumn<T> = {
  align?: "left" | "right";
  header: ReactNode;
  key: string;
  /**
   * Where this column goes in the phone layout, when the default is wrong.
   *
   * By default the first column becomes the card's title, an "操作" column
   * becomes its footer, and everything else becomes a labelled row.
   */
  mobile?: "title" | "field" | "actions" | "hidden";
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

/**
 * Assigns each column a place on a card.
 *
 * A table row reads left to right with the header row for context; a card has
 * no header row, so the identifying column has to become the title and the
 * rest have to carry their own labels. The defaults match how every table in
 * the app is written — identity first, buttons under "操作" — and `mobile`
 * overrides them where that does not hold.
 */
function splitForCards<T>(columns: DataTableColumn<T>[]) {
  const actionColumns: DataTableColumn<T>[] = [];
  const fieldColumns: DataTableColumn<T>[] = [];
  let titleColumn: DataTableColumn<T> | undefined;

  columns.forEach((column, index) => {
    const placement =
      column.mobile ??
      (column.header === "操作" || column.header === ""
        ? "actions"
        : index === 0
          ? "title"
          : "field");

    if (placement === "hidden") {
      return;
    }
    if (placement === "actions") {
      actionColumns.push(column);
    } else if (placement === "title" && !titleColumn) {
      titleColumn = column;
    } else {
      fieldColumns.push(column);
    }
  });

  return { actionColumns, fieldColumns, titleColumn };
}

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
  const isMobile = useIsMobile();
  const selection = new Set(selectedKeys ?? []);
  const pageKeys = items.map(getRowKey);
  const { actionColumns, fieldColumns, titleColumn } = splitForCards(columns);
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

  if (isMobile) {
    return (
      <ul className="data-cards">
        {items.map((item) => {
          const key = getRowKey(item);
          return (
            <li className="data-card" key={key}>
              {/* The whole card takes a tap, but only the title is a control:
                  a card-sized button would announce its every field as one
                  long name, and keyboard users would have no way past it. */}
              <div
                className={onRowClick ? "data-card-main clickable" : "data-card-main"}
                onClick={onRowClick ? () => onRowClick(item) : undefined}
              >
                {selectable ? (
                  <span
                    className="data-card-select"
                    onClick={(event) => event.stopPropagation()}
                  >
                    <input
                      aria-label="选择此行"
                      checked={selection.has(key)}
                      type="checkbox"
                      onChange={(event) => toggleRow(key, event.target.checked)}
                    />
                  </span>
                ) : null}
                <div className="data-card-body">
                  {titleColumn ? (
                    onRowClick ? (
                      <button
                        className="data-card-title"
                        type="button"
                        onClick={() => onRowClick(item)}
                      >
                        {titleColumn.render(item)}
                      </button>
                    ) : (
                      <div className="data-card-title">{titleColumn.render(item)}</div>
                    )
                  ) : null}
                  <dl className="data-card-fields">
                    {fieldColumns.map((column) => (
                      <div className="data-card-field" key={column.key}>
                        <dt>{column.header}</dt>
                        <dd>{column.render(item)}</dd>
                      </div>
                    ))}
                  </dl>
                </div>
              </div>
              {actionColumns.length > 0 ? (
                // The card navigates; its buttons must not also navigate.
                <div
                  className="data-card-actions"
                  onClick={(event) => event.stopPropagation()}
                >
                  {actionColumns.map((column) => (
                    <span key={column.key}>{column.render(item)}</span>
                  ))}
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
    );
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
