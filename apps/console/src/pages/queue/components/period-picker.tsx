import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, m } from "motion/react";
import { useRef, useState } from "react";

import { Button } from "@cihub/ui/components/button";
import { transitions } from "@cihub/ui/components/motion";
import { RangeCalendar } from "@cihub/ui/components/range-calendar";
import { Select } from "@cihub/ui/components/select";
import { useCloseOutside } from "@cihub/ui/hooks/use-close-outside";
import { cn } from "@cihub/ui/lib/cn";
import {
  type HealthPeriod,
  grainOf,
  hasPeriodBefore,
  oldestDay,
  presetsOf,
  rangeText,
  spanText,
} from "@/domain/queue/period";
import type { QueuePath } from "@/domain/queue/queries";
import {
  DAY_MS,
  HOUR_MS,
  addMonths,
  hourText,
  startOfDay,
  startOfMonth,
  utcText,
} from "@cihub/utils/utc-days";

/** The period that the person builds in the calendar, before Apply. */
interface Draft {
  first: number | null;
  last: number | null;
  hasHours: boolean;
  fromHour: number;
  toHour: number;
  /** The first of the two months shown. */
  month: number;
  /** The dates as typed, until they read as days. */
  typed: { from: string; to: string } | null;
  invalid: "from" | "to" | null;
}

const PRESET_DAYS = { "24h": 1, "7d": 6, "30d": 29 };

function draftOf(period: HealthPeriod, now: number): Draft {
  const today = startOfDay(now);
  if ("range" in period) {
    return {
      first: today - PRESET_DAYS[period.range] * DAY_MS,
      last: today,
      hasHours: false,
      fromHour: 0,
      toHour: 24,
      month: addMonths(today, -1),
      typed: null,
      invalid: null,
    };
  }
  const [from, to] = [Date.parse(period.from), Date.parse(period.to)];
  const hasHours = from % DAY_MS !== 0 || to % DAY_MS !== 0;
  const last = startOfDay(to - 1);
  return {
    first: startOfDay(from),
    last,
    hasHours,
    fromHour: new Date(from).getUTCHours(),
    toHour: to % DAY_MS === 0 ? 24 : new Date(to).getUTCHours(),
    month: addMonths(startOfMonth(last), -1),
    typed: null,
    invalid: null,
  };
}

/** The period of a draft, its end excluded, once its two days are chosen. */
function periodOf(draft: Draft): { from: number; to: number } | null {
  if (draft.first === null || draft.last === null) {
    return null;
  }
  const from = draft.first + (draft.hasHours ? draft.fromHour : 0) * HOUR_MS;
  const to = draft.last + (draft.hasHours ? draft.toHour : 24) * HOUR_MS;
  return to > from ? { from, to } : null;
}

const dayText = (day: number | null) =>
  day === null ? "" : utcText(day, { month: "short", day: "numeric", year: "numeric" });

/**
 * The Custom choice of the periods of the Health: a calendar of two months, its shortcuts and the hours, in UTC.
 * Apply puts the period in the address of the page. `Escape` or a click beside it closes it without a change.
 */
export function PeriodPicker({ path, period, now }: { path: QueuePath; period: HealthPeriod; now: number }) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const container = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const navigate = useNavigate();
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

function Calendar({
  draft,
  now,
  onChange,
  onApply,
}: {
  draft: Draft;
  now: number;
  onChange: (draft: Draft | null) => void;
  onApply: (period: { from: number; to: number }) => void;
}) {
  const today = startOfDay(now);
  const oldest = oldestDay(now);
  const presets = presetsOf(now);
  const chosen = periodOf(draft);
  const activePreset = draft.hasHours
    ? undefined
    : presets.find((preset) => preset.first === draft.first && preset.last === draft.last)?.id;

  const pick = (day: number) => {
    const isNew = draft.first === null || draft.last !== null;
    onChange({
      ...draft,
      first: isNew ? day : Math.min(draft.first ?? day, day),
      last: isNew ? null : Math.max(draft.first ?? day, day),
      typed: null,
      invalid: null,
    });
  };
  // A date typed in a field moves the calendar to it; a date that does not read, or out of the 13 months, shows in red.
  const type = (field: "from" | "to", value: string) => {
    const typed = { from: dayText(draft.first), to: dayText(draft.last), ...draft.typed, [field]: value };
    const parsed = Date.parse(`${value} UTC`);
    const day = Number.isNaN(parsed) ? null : startOfDay(parsed);
    if (day === null || day > today || day < oldest) {
      onChange({ ...draft, typed, invalid: field });
      return;
    }
    const [first, last] = field === "from" ? [day, draft.last] : [draft.first, day];
    const ordered = first !== null && last !== null && first > last ? [last, first] : [first, last];
    onChange({
      ...draft,
      first: ordered[0] ?? null,
      last: ordered[1] ?? null,
      month: addMonths(startOfMonth(ordered[1] ?? day), -1),
      typed: null,
      invalid: null,
    });
  };

  return (
    <>
      <div
        role="group"
        aria-label="Shortcuts"
        className="grid content-start gap-px border-r border-divider p-1.5 max-md:flex max-md:gap-1 max-md:overflow-x-auto max-md:border-r-0 max-md:border-b"
      >
        {presets.map((preset) => (
          <button
            key={preset.id}
            type="button"
            aria-pressed={preset.id === activePreset}
            className={cn(
              "h-7 cursor-pointer rounded-sm px-2 text-left whitespace-nowrap text-secondary-foreground hover:bg-muted hover:text-foreground max-md:flex-none max-md:rounded-full max-md:border max-md:border-border",
              preset.id === activePreset &&
                "bg-primary-muted font-medium text-primary hover:bg-primary-muted",
            )}
            onClick={() => {
              onChange({
                ...draft,
                first: preset.first,
                last: preset.last,
                hasHours: false,
                fromHour: 0,
                toHour: 24,
                month: addMonths(startOfMonth(preset.last), -1),
                typed: null,
                invalid: null,
              });
            }}
          >
            {preset.label}
          </button>
        ))}
      </div>
      <div className="grid gap-3 px-3.5 py-3">
        <div className="flex items-end gap-2 max-md:flex-wrap">
          <DateField
            key={`from-${draft.typed?.from ?? dayText(draft.first)}`}
            label="Start"
            value={draft.typed?.from ?? dayText(draft.first)}
            isInvalid={draft.invalid === "from"}
            hour={draft.hasHours ? { value: draft.fromHour, first: 0 } : null}
            onType={(value) => {
              type("from", value);
            }}
            onHour={(fromHour) => {
              onChange({ ...draft, fromHour });
            }}
          />
          <span
            aria-hidden="true"
            className="grid h-7.5 place-items-center text-muted-foreground max-md:hidden"
          >
            →
          </span>
          <DateField
            key={`to-${draft.typed?.to ?? dayText(draft.last)}`}
            label="End"
            value={draft.typed?.to ?? dayText(draft.last)}
            placeholder={draft.last === null ? "Pick a day" : undefined}
            isInvalid={draft.invalid === "to"}
            hour={draft.hasHours ? { value: draft.toHour, first: 1 } : null}
            onType={(value) => {
              type("to", value);
            }}
            onHour={(toHour) => {
              onChange({ ...draft, toHour });
            }}
          />
        </div>
        <RangeCalendar
          month={draft.month}
          onMonthChange={(month) => {
            onChange({ ...draft, month });
          }}
          first={draft.first}
          last={draft.last}
          min={{ day: oldest, why: "Older than 13 months: CIHub no longer keeps it" }}
          max={{ day: today, why: "In the future" }}
          marked={today}
          onPick={pick}
        />
        <div className="flex items-center gap-3 border-t border-divider pt-2.5 text-md text-secondary-foreground max-md:flex-wrap">
          <label className="inline-flex cursor-pointer items-center gap-1.5 whitespace-nowrap">
            <input
              type="checkbox"
              className="m-0 accent-primary"
              checked={draft.hasHours}
              onChange={() => {
                onChange({ ...draft, hasHours: !draft.hasHours });
              }}
            />
            Choose the hours
          </label>
          {/* No width of its own: the calendar sets the width of the dialog, and the summary wraps in it. */}
          <span className="w-0 flex-1 text-right text-muted-foreground max-md:w-auto max-md:flex-initial max-md:text-left">
            <Summary draft={draft} chosen={chosen} now={now} />
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="mr-auto text-sm text-muted-foreground">Times in UTC. CIHub keeps 13 months.</span>
          <Button
            onClick={() => {
              onChange(null);
            }}
          >
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={chosen === null}
            onClick={() => {
              if (chosen) {
                onApply(chosen);
              }
            }}
          >
            Apply
          </Button>
        </div>
      </div>
    </>
  );
}

/** The length of the period, the size of its bars and the period that it is compared with. */
function Summary({
  draft,
  chosen,
  now,
}: {
  draft: Draft;
  chosen: { from: number; to: number } | null;
  now: number;
}) {
  if (!chosen) {
    return draft.first === null ? "Pick the first day" : "Pick the last day";
  }
  // The server reads up to the end of the current hour.
  const to = Math.min(chosen.to, Math.ceil(now / HOUR_MS) * HOUR_MS);
  const span = to - chosen.from;

  return (
    <>
      <b className="font-medium text-foreground">{spanText(span)}</b> · charts by {grainOf(span)} ·{" "}
      {hasPeriodBefore(chosen.from, to, now)
        ? `compared with ${rangeText(chosen.from - span, chosen.from, now)}`
        : "no period before to compare with: older than 13 months"}
    </>
  );
}

function DateField({
  label,
  value,
  placeholder,
  isInvalid,
  hour,
  onType,
  onHour,
}: {
  label: string;
  value: string;
  placeholder?: string;
  isInvalid: boolean;
  hour: { value: number; first: number } | null;
  onType: (value: string) => void;
  onHour: (hour: number) => void;
}) {
  // The parent gives the field the key of its value: a new value starts the field again.
  const [text, setText] = useState(value);

  return (
    <label className="grid flex-1 gap-1">
      <span className="text-sm text-secondary-foreground">{label}</span>
      <span
        className={cn(
          "flex h-7.5 items-center overflow-hidden rounded-sm border border-border bg-card focus-within:border-primary",
          isInvalid && "border-destructive focus-within:border-destructive",
        )}
      >
        <input
          value={text}
          placeholder={placeholder}
          aria-invalid={isInvalid}
          spellCheck={false}
          autoComplete="off"
          className="h-full min-w-0 flex-1 bg-transparent px-2 text-foreground tabular-nums outline-none placeholder:text-muted-foreground"
          onChange={(event) => {
            setText(event.target.value);
          }}
          onBlur={() => {
            if (text !== value) {
              onType(text);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              onType(text);
            }
          }}
        />
        {hour && (
          <Select
            aria-label={`${label} hour`}
            className="h-full rounded-none border-0 border-l border-divider bg-muted text-md tabular-nums"
            value={hour.value}
            onChange={(event) => {
              onHour(Number(event.target.value));
            }}
          >
            {Array.from({ length: 24 }, (_, index) => index + hour.first).map((option) => (
              <option key={option} value={option}>
                {hourText(option)}
              </option>
            ))}
          </Select>
        )}
      </span>
    </label>
  );
}
