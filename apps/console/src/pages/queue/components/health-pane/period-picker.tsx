import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, m } from "motion/react";
import { useRef, useState } from "react";

import { transitions } from "@cihub/ui/components/motion";
import { useCloseOutside } from "@cihub/ui/hooks/use-close-outside";
import { cn } from "@cihub/ui/lib/cn";
import { type HealthPeriod, rangeText } from "@/domain/queue/period";
import type { QueuePath } from "@/domain/queue/queries";
import { Calendar } from "./calendar";
import { type Draft, draftOf } from "./period-draft";

/**
 * The Custom choice of the periods of the Health: a calendar of two months, its shortcuts and the hours, in UTC.
 * Apply puts the period in the address of the page. `Escape` or a click beside it closes it without a change.
 */
export function PeriodPicker({ path, period, now }: { path: QueuePath; period: HealthPeriod; now: number }) {
  const navigate = useNavigate();

  const [draft, setDraft] = useState<Draft | null>(null);
  const container = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  const isCustom = !("range" in period);
  const isOpen = draft !== null;

  useCloseOutside(container, trigger, isOpen, () => {
    setDraft(null);
  });

  const apply = (chosen: { from: number; to: number }) => {
    setDraft(null);
    void navigate({
      to: "/orgs/$org/repos/$repo/queues/$queue",
      params: path,
      search: {
        pane: "health",
        from: new Date(chosen.from).toISOString(),
        to: new Date(chosen.to).toISOString(),
      },
    });
  };

  return (
    <div ref={container} className="contents">
      <button
        type="button"
        ref={trigger}
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        aria-current={isCustom ? "page" : undefined}
        className={cn(
          "flex h-6 cursor-pointer items-center justify-center gap-1.5 rounded-xs px-2.5 text-md whitespace-nowrap text-secondary-foreground hover:text-foreground max-md:flex-1 [&>svg]:size-3.25 [&>svg]:flex-none",
          isCustom && "bg-muted font-medium text-foreground",
        )}
        onClick={() => {
          setDraft(isOpen ? null : draftOf(period, now));
        }}
      >
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect x="2.5" y="3.5" width="11" height="10" rx="2" stroke="currentColor" strokeWidth="1.4" />
          <path
            d="M2.5 6.75 H13.5 M5.5 2 V4.5 M10.5 2 V4.5"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
          />
        </svg>
        {isCustom ? rangeText(Date.parse(period.from), Date.parse(period.to), now) : "Custom"}
      </button>
      <AnimatePresence>
        {draft && (
          <m.div
            key="calendar"
            role="dialog"
            aria-label="Choose a period"
            className="absolute top-[calc(100%_+_6px)] right-0 z-60 grid w-max grid-cols-[148px_minmax(0,1fr)] rounded-md border border-border bg-card text-base shadow-lg max-md:fixed max-md:top-16 max-md:right-3 max-md:left-3 max-md:max-h-[calc(100vh_-_80px)] max-md:w-auto max-md:grid-cols-1 max-md:overflow-y-auto"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={transitions.fade}
          >
            <Calendar draft={draft} now={now} onChange={setDraft} onApply={apply} />
          </m.div>
        )}
      </AnimatePresence>
    </div>
  );
}
