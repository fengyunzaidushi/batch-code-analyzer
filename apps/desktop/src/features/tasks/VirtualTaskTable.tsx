import { useMemo, useState, type ReactNode } from "react";

export interface VirtualTaskTableProps<T> {
  items: readonly T[];
  getRowKey: (item: T, index: number) => string;
  renderRow: (item: T, index: number) => ReactNode;
  header: ReactNode;
  ariaLabel?: string;
  className?: string;
  emptyLabel?: string;
  rowHeight?: number;
  viewportHeight?: number;
}

/** Fixed-height virtual rows keep large task lists responsive without loading all cells. */
export function VirtualTaskTable<T>({
  items,
  getRowKey,
  renderRow,
  header,
  ariaLabel,
  className,
  emptyLabel = "暂无任务",
  rowHeight = 56,
  viewportHeight = 424,
}: VirtualTaskTableProps<T>) {
  const [scrollTop, setScrollTop] = useState(0);
  const overscan = 5;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  const end = Math.min(items.length, start + visibleCount);
  const visibleItems = useMemo(
    () => items.slice(start, end),
    [end, items, start],
  );

  return (
    <div
      className={`task-table${className ? ` ${className}` : ""}`}
      role="table"
      aria-label={ariaLabel}
      aria-rowcount={items.length}
    >
      <div className="task-table-header" role="row">
        {header}
      </div>
      {items.length === 0 ? (
        <div className="task-table-empty" role="row">
          {emptyLabel}
        </div>
      ) : (
        <div
          className="task-table-viewport"
          style={{ height: viewportHeight }}
          onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        >
          <div
            className="task-table-canvas"
            style={{ height: items.length * rowHeight }}
          >
            <div
              className="task-table-window"
              style={{ transform: `translateY(${start * rowHeight}px)` }}
            >
              {visibleItems.map((item, offset) => {
                const index = start + offset;
                return (
                  <div
                    className="task-table-row"
                    key={getRowKey(item, index)}
                    role="row"
                    style={{ minHeight: rowHeight }}
                  >
                    {renderRow(item, index)}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
