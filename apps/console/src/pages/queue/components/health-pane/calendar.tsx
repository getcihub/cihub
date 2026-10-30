import { Button } from "@cihub/ui/components/button";
import { RangeCalendar } from "@cihub/ui/components/range-calendar";
import { cn } from "@cihub/ui/lib/cn";
import { oldestDay, presetsOf } from "@/domain/queue/period";
import { addMonths, startOfDay, startOfMonth } from "@cihub/utils/utc-days";
import { DateField, Summary } from "./date-field";
import { type Draft, dayText, periodOf } from "./period-draft";

export function Calendar({
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
