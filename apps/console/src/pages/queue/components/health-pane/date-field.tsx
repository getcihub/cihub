import { useState } from "react";

import { Select } from "@cihub/ui/components/select";
import { cn } from "@cihub/ui/lib/cn";
import { grainOf, hasPeriodBefore, rangeText, spanText } from "@/domain/queue/period";
import { HOUR_MS, hourText } from "@cihub/utils/utc-days";
import type { Draft } from "./period-draft";

/** The length of the period, the size of its bars and the period that it is compared with. */
export function Summary({
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

export function DateField({
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
