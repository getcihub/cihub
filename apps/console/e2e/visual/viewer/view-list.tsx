import { useEffect, useRef } from "react";

import { Badge } from "@cihub/ui/components/badge";
import { Input } from "@cihub/ui/components/input";
import { cn } from "@cihub/ui/lib/cn";
import { type ViewDifference, type ViewStatus, partsOf } from "./data";

export const STATUSES: {
  status: ViewStatus;
  label: string;
  tone: "warning" | "success" | "destructive";
}[] = [
  { status: "changed", label: "Changed", tone: "warning" },
  { status: "added", label: "New", tone: "success" },
  { status: "removed", label: "Removed", tone: "destructive" },
];

const SHORTCUTS: [string, string][] = [
  ["↑ ↓", "views"],
  ["← →", "before, after"],
  ["1–4", "modes"],
  ["Z", "zoom"],
];

function Row({
  view,
  isSelected,
  onSelect,
}: {
  view: ViewDifference;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const { page, variant } = partsOf(view.name);
  const row = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (isSelected) row.current?.scrollIntoView({ block: "nearest" });
  }, [isSelected]);

  return (
    <button
      ref={row}
      type="button"
      onClick={onSelect}
      aria-current={isSelected || undefined}
      title={view.name}
      className={cn(
        "flex w-full cursor-pointer items-baseline gap-2 rounded-sm px-2.5 py-1.5 text-left hover:bg-muted",
        isSelected && "bg-primary-muted hover:bg-primary-muted",
      )}
    >
      <span className="min-w-0 flex-1">
        <span className={cn("block truncate text-base font-medium", isSelected && "text-primary")}>
          {page}
        </span>
        {variant && <span className="block text-sm text-muted-foreground">{variant}</span>}
      </span>
      {view.pixels !== null && (
        <span className="flex-none text-sm text-muted-foreground tabular-nums">
          {view.pixels.toLocaleString("en")} px
        </span>
      )}
    </button>
  );
}

/** The views by status, the changed ones first, and the filter that narrows them. */
export function ViewList({
  views,
  selected,
  filter,
  onFilter,
  onSelect,
}: {
  views: ViewDifference[];
  selected: string | undefined;
  filter: string;
  onFilter: (filter: string) => void;
  onSelect: (name: string) => void;
}) {
  return (
    <aside className="flex min-h-0 flex-col border-r border-border bg-card">
      <div className="border-b border-divider p-3">
        <Input
          type="search"
          value={filter}
          onChange={(event) => {
            onFilter(event.target.value);
          }}
          placeholder="Filter the views"
          aria-label="Filter the views"
          className="w-full"
        />
      </div>

      <nav className="min-h-0 flex-1 overflow-auto p-2" aria-label="Views">
        {STATUSES.map(({ status, label, tone }) => {
          const group = views.filter((view) => view.status === status);
          if (group.length === 0) return null;

          return (
            <section key={status} className="mb-3">
              <h2 className="flex items-center justify-between px-2.5 py-1.5">
                <Badge tone={tone} className="text-sm text-secondary-foreground">
                  {label}
                </Badge>
                <span className="text-sm text-muted-foreground tabular-nums">{group.length}</span>
              </h2>
              {group.map((view) => (
                <Row
                  key={view.name}
                  view={view}
                  isSelected={view.name === selected}
                  onSelect={() => {
                    onSelect(view.name);
                  }}
                />
              ))}
            </section>
          );
        })}
        {views.length === 0 && (
          <p className="px-2.5 py-1.5 text-base text-muted-foreground">No view matches.</p>
        )}
      </nav>

      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 border-t border-divider px-4 py-3 text-sm">
        {SHORTCUTS.map(([keys, action]) => (
          <div key={keys} className="contents">
            <dt>
              <kbd className="font-mono text-secondary-foreground">{keys}</kbd>
            </dt>
            <dd className="text-muted-foreground">{action}</dd>
          </div>
        ))}
      </dl>
    </aside>
  );
}
