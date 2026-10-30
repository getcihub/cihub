"use client";

import { useState } from "react";

import { cn } from "../lib/cn";
import { DAY_MS, addMonths, utcText } from "@cihub/utils/utc-days";

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

interface Props {
  /** The first of the two months shown: the UTC milliseconds of its first day. */
  month: number;
  onMonthChange: (month: number) => void;
  /** The first and the last chosen days, as the UTC milliseconds of their start; `last` is null while it is picked. */
  first: number | null;
  last: number | null;
  /** The days that can be picked, and why the others cannot, in their title. */
  min: { day: number; why: string };
  max: { day: number; why: string };
  /** A day that a dot marks: today. */
  marked?: number;
  onPick: (day: number) => void;
}

/**
 * Two months in UTC to pick the first and the last day of a period. While the last day is picked, the days under
 * the pointer show the period. A narrow screen shows the second month only.
 */
export function RangeCalendar({ month, onMonthChange, first, last, min, max, marked, onPick }: Props) {
  const [hover, setHover] = useState<number | null>(null);
  const [low, high] =
    first !== null && last === null && hover !== null
      ? [Math.min(first, hover), Math.max(first, hover)]
      : [first, last ?? first];
  const canGoBack = month > min.day;
  const canGoForward = addMonths(month, 2) <= max.day;
  const grid = (shown: number, className?: string) => (
    <MonthGrid
      month={shown}
      range={[low, high]}
      min={min}
      max={max}
      marked={marked}
      className={className}
      onPick={onPick}
      onHover={setHover}
    />
  );

  return (
    <div
      data-slot="range-calendar"
      className="relative grid grid-cols-[repeat(2,224px)] gap-5 max-md:grid-cols-1"
      onMouseLeave={() => {
        setHover(null);
      }}
    >
      <NavButton
        label="Previous month"
        className="left-0 -scale-x-100"
        disabled={!canGoBack}
        onClick={() => {
          onMonthChange(addMonths(month, -1));
        }}
      />
      {grid(month, "max-md:hidden")}
      {grid(addMonths(month, 1))}
      <NavButton
        label="Next month"
        className="right-0"
        disabled={!canGoForward}
        onClick={() => {
          onMonthChange(addMonths(month, 1));
        }}
      />
    </div>
  );
}

function NavButton({
  label,
  className,
  disabled,
  onClick,
}: {
  label: string;
  className: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      className={cn(
        "absolute top-0 grid size-7 cursor-pointer place-items-center rounded-sm text-secondary-foreground hover:bg-muted hover:text-foreground disabled:cursor-default disabled:opacity-35 disabled:hover:bg-transparent [&>svg]:size-3.5",
        className,
      )}
      onClick={onClick}
    >
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M6 3.5 L10.5 8 L6 12.5"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </button>
  );
}

function MonthGrid({
  month,
  range: [low, high],
  min,
  max,
  marked,
  className,
  onPick,
  onHover,
}: {
  month: number;
  range: [number | null, number | null];
  min: Props["min"];
  max: Props["max"];
  marked: number | undefined;
  className?: string;
  onPick: (day: number) => void;
  onHover: (day: number | null) => void;
}) {
  const offset = (new Date(month).getUTCDay() + 6) % 7;
  const days = Math.round((addMonths(month, 1) - month) / DAY_MS);

  return (
    <div className={className}>
      <div className="grid h-7 place-items-center font-medium">
        {utcText(month, { month: "long", year: "numeric" })}
      </div>
      <div className="grid grid-cols-[repeat(7,32px)] gap-y-0.5 max-md:grid-cols-7">
        {WEEKDAYS.map((name) => (
          <span key={name} className="grid h-6.5 place-items-center text-2xs text-muted-foreground">
            {name}
          </span>
        ))}
        {Array.from({ length: offset }, (_, index) => (
          <span key={`blank-${String(index)}`} />
        ))}
        {Array.from({ length: days }, (_, index) => {
          const day = month + index * DAY_MS;
          const why = day < min.day ? min.why : day > max.day ? max.why : null;
          const isIn = low !== null && high !== null && day >= low && day <= high;
          const isEdge = day === low || day === high;

          return (
            <button
              key={day}
              type="button"
              disabled={why !== null}
              title={why ?? utcText(day, { weekday: "long", month: "long", day: "numeric", year: "numeric" })}
              aria-pressed={isEdge}
              className={cn(
                "relative h-7.5 cursor-pointer text-md text-foreground tabular-nums hover:rounded-sm hover:bg-muted disabled:cursor-default disabled:text-muted-foreground disabled:opacity-45 disabled:hover:bg-transparent",
                isIn && "bg-primary-muted hover:rounded-none hover:bg-primary-muted",
                isEdge && "bg-primary font-medium text-primary-foreground hover:bg-primary",
                day === low && "rounded-l-sm hover:rounded-l-sm",
                day === high && "rounded-r-sm hover:rounded-r-sm",
                day === marked &&
                  "after:absolute after:bottom-1 after:left-1/2 after:size-0.75 after:-translate-x-1/2 after:rounded-full after:bg-primary after:content-['']",
                day === marked && isEdge && "after:bg-primary-foreground",
              )}
              onClick={() => {
                onPick(day);
              }}
              onMouseEnter={() => {
                onHover(why === null ? day : null);
              }}
            >
              {index + 1}
            </button>
          );
        })}
      </div>
    </div>
  );
}
