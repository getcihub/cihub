import type { KeyboardEvent } from "react";

import { cn } from "@/lib/cn";

/** An option of a segmented control. */
export interface Segment<T extends string> {
  value: T;
  label: string;
  /** What the option means, in a tooltip. */
  title?: string;
}

/**
 * A choice among two or three options side by side: a radio group. Only the chosen option takes the Tab, and the
 * arrows move the choice, as in a native radio group.
 */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled = false,
}: {
  label: string;
  value: T;
  options: Segment<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined) {
      return;
    }
    event.preventDefault();
    const next = options[(index + step + options.length) % options.length];
    if (next) {
      onChange(next.value);
      const group = event.currentTarget.parentElement;
      group?.querySelectorAll<HTMLButtonElement>("[role=radio]")[options.indexOf(next)]?.focus();
    }
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      data-slot="segmented"
      className="inline-flex h-7 flex-none gap-0.5 rounded-sm border border-border bg-muted p-0.5"
    >
      {options.map((option, index) => {
        const isChosen = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={isChosen}
            tabIndex={isChosen ? 0 : -1}
            title={option.title}
            disabled={disabled}
            onClick={() => {
              onChange(option.value);
            }}
            onKeyDown={(event) => {
              onKeyDown(event, index);
            }}
            className={cn(
              "cursor-pointer rounded-xs px-2.5 text-base text-secondary-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary disabled:cursor-default",
              isChosen && "bg-card font-medium text-primary shadow-chosen",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
