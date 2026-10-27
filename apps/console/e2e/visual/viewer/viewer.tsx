import { useEffect, useMemo, useState } from "react";

import { Badge, Button, Card, CardHeader, CardTitle, cn, EmptyState, Mark } from "@cihub/ui";
import { toggleTheme } from "@/lib/preferences";
import { Comparison, type Mode, type Side, type Size, isForViewer } from "./comparison";
import { type ViewDifference, type ViewerData, partsOf } from "./data";
import { STATUSES, ViewList } from "./view-list";

const MODES: { value: Mode; label: string; key: string }[] = [
  { value: "side", label: "Side by side", key: "1" },
  { value: "split", label: "Split", key: "2" },
  { value: "blend", label: "Blend", key: "3" },
  { value: "changes", label: "Changes", key: "4" },
];

const SIZES: { value: Size; label: string }[] = [
  { value: "change", label: "The change" },
  { value: "fit", label: "Fit" },
  { value: "actual", label: "100%" },
];

const ORDER = STATUSES.map(({ status }) => status);

/** The view and the mode of the address: `#view=queue-live-light-1280.png&mode=split`. */
function fromHash(): { view: string | null; mode: Mode | undefined } {
  const params = new URLSearchParams(location.hash.slice(1));
  const mode = MODES.find(({ value }) => value === params.get("mode"))?.value;
  return { view: params.get("view"), mode };
}

function Segments<T extends string>({
  label,
  options,
  value,
  isDisabled,
  onChange,
}: {
  label: string;
  options: { value: T; label: string; key?: string }[];
  value: T;
  isDisabled: (value: T) => boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div role="group" aria-label={label} className="flex gap-1">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          disabled={isDisabled(option.value)}
          aria-pressed={option.value === value && !isDisabled(option.value)}
          onClick={() => {
            onChange(option.value);
          }}
          className={cn(
            "cursor-pointer rounded-sm px-2.5 py-1 text-base font-medium text-secondary-foreground hover:text-foreground disabled:cursor-default disabled:opacity-40 disabled:hover:text-secondary-foreground",
            option.value === value && !isDisabled(option.value) && "bg-muted text-primary hover:text-primary",
          )}
        >
          {option.label}
          {option.key && <kbd className="ml-1.5 font-mono text-sm text-muted-foreground">{option.key}</kbd>}
        </button>
      ))}
    </div>
  );
}

function detailOf(view: ViewDifference): string {
  if (view.status !== "changed") {
    return STATUSES.find(({ status }) => status === view.status)?.label ?? "";
  }
  if (!view.box || view.pixels === null) {
    return "";
  }

  const { left, right, top, bottom } = view.box;
  const area = `x ${String(left)}–${String(right)}, y ${String(top)}–${String(bottom)}`;
  const size = view.resized ? ` · size ${view.resized}` : "";
  return `${view.pixels.toLocaleString("en")} pixels changed · ${area}${size}`;
}

/** The captures of two builds, view by view: what changed first, before and after. */
export function Viewer({ data }: { data: ViewerData }) {
  const views = useMemo(
    () =>
      data.views.toSorted(
        (left, right) =>
          ORDER.indexOf(left.status) - ORDER.indexOf(right.status) ||
          partsOf(left.name).page.localeCompare(partsOf(right.name).page) ||
          left.name.localeCompare(right.name),
      ),
    [data],
  );
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState(() => fromHash().view ?? views[0]?.name);
  const [mode, setMode] = useState<Mode>(() => fromHash().mode ?? "side");
  const [size, setSize] = useState<Size>("change");
  const [side, setSide] = useState<Side | null>(null);
  const shown = views.filter((view) => view.name.includes(filter.trim()));
  const view = views.find((candidate) => candidate.name === selected) ?? views[0];
  const isCompared = view?.status === "changed";

  const select = (name: string) => {
    setSelected(name);
    setSide(null);
  };

  useEffect(() => {
    if (!view) return;

    history.replaceState(null, "", `#${new URLSearchParams({ view: view.name, mode }).toString()}`);
  }, [view, mode]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey || !isForViewer(event)) return;

      const index = shown.findIndex((candidate) => candidate === view);
      const step = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 }[event.key];
      const nextMode = MODES.find(({ key }) => key === event.key)?.value;
      const nextSide = { ArrowLeft: "before", ArrowRight: "after" }[event.key] as Side | undefined;

      if (step && shown.length > 0) {
        const next = shown[(index + step + shown.length) % shown.length];
        if (next) {
          setSelected(next.name);
          setSide(null);
        }
      } else if (nextMode && isCompared) {
        setMode(nextMode);
        setSide(null);
      } else if (nextSide && view?.status === "changed") {
        setSide(nextSide);
      } else if (event.key === "Escape") {
        setSide(null);
      } else if (event.key === "z" && isCompared) {
        setSize((current) => (current === "change" ? "fit" : "change"));
      } else {
        return;
      }
      event.preventDefault();
    };

    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [shown, view, isCompared]);

  const changed = data.views.filter(({ status }) => status === "changed").length;
  const { page, variant } = view ? partsOf(view.name) : { page: "", variant: "" };

  return (
    <div className="grid h-screen grid-rows-[auto_minmax(0,1fr)]">
      <header className="flex h-13 items-center gap-3 border-b border-border bg-card px-4">
        <Mark className="size-6" />
        <h1 className="text-xl font-medium">Visual changes</h1>
        <span className="text-base text-muted-foreground">
          {changed} of {data.compared} views differ from {data.label.replaceAll("`", "")}
        </span>
        <div className="ml-auto flex items-center gap-4">
          {STATUSES.map(({ status, label, tone }) => {
            const count = data.views.filter((candidate) => candidate.status === status).length;
            return (
              count > 0 && (
                <Badge key={status} tone={tone}>
                  {count} {label.toLowerCase()}
                </Badge>
              )
            );
          })}
          <Button onClick={toggleTheme}>Light / dark</Button>
        </div>
      </header>

      <div className="grid min-h-0 grid-cols-[288px_1fr]">
        <ViewList
          views={shown}
          selected={view?.name}
          filter={filter}
          onFilter={setFilter}
          onSelect={select}
        />

        <main className="min-w-0 overflow-auto p-6">
          {view ? (
            <Card>
              <CardHeader className="flex-wrap py-3">
                <div className="mr-auto min-w-0">
                  <div className="flex items-baseline gap-2">
                    <CardTitle className="font-mono">{page}</CardTitle>
                    {variant && <span className="text-base text-muted-foreground">{variant}</span>}
                  </div>
                  <div className="text-md text-secondary-foreground">{detailOf(view)}</div>
                </div>
                <Segments
                  label="Mode"
                  options={MODES}
                  value={mode}
                  isDisabled={() => !isCompared}
                  onChange={(value) => {
                    setMode(value);
                    setSide(null);
                  }}
                />
                <span className="h-5 w-px bg-divider" />
                <Segments
                  label="Size"
                  options={SIZES}
                  value={isCompared || size !== "change" ? size : "fit"}
                  isDisabled={(value) => value === "change" && !isCompared}
                  onChange={setSize}
                />
              </CardHeader>
              <div className="p-4">
                <Comparison
                  key={view.name}
                  view={view}
                  paths={{
                    before: `before/${view.name}`,
                    after: `after/${view.name}`,
                    changes: `changes/${view.name}`,
                  }}
                  mode={mode}
                  size={size}
                  side={side}
                />
              </div>
            </Card>
          ) : (
            <Card>
              <EmptyState illustration={null} title="No view to compare">
                The two folders hold no capture.
              </EmptyState>
            </Card>
          )}
        </main>
      </div>
    </div>
  );
}
