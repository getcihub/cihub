import { type KeyboardEvent, type ReactNode, useId, useRef, useState } from "react";

import { cn } from "@/lib/cn";
import { useCloseOutside } from "./hooks/use-close-outside";

/** An option of a picker: its value, what the button and the list say, and a description under it. */
export interface PickerOption<T extends string> {
  value: T;
  label: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
}

function Chevron() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M4.5 6.5 L8 10 L11.5 6.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function Check() {
  return (
    <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path d="M5 10.5 L8.5 14 L15 6.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

/**
 * A choice among a few options, each with a description: a button that opens a list. The arrows move in the list,
 * Enter or Space chooses, Escape closes it and gives the focus back to the button, Tab and a click outside close
 * it. An arrow on the closed button opens it.
 */
export function OptionPicker<T extends string>({
  label,
  value,
  options,
  onChange,
  isWide = false,
  align = "start",
  className,
}: {
  /** What the picker chooses, as a screen reader names it: "Merge method". */
  label: string;
  value: T;
  options: PickerOption<T>[];
  onChange: (value: T) => void;
  /** The picker takes the width of its parent. */
  isWide?: boolean;
  /** `end` opens the list toward the left, for a picker at the right edge. */
  align?: "start" | "end";
  className?: string;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [active, setActive] = useState(0);
  const container = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const listId = useId();
  const current = options.find((option) => option.value === value) ?? options[0];
  const enabled = options.filter((option) => !option.disabled);

  const open = () => {
    setActive(
      Math.max(
        0,
        enabled.findIndex((option) => option.value === value),
      ),
    );
    setIsOpen(true);
  };
  const choose = (option: PickerOption<T> | undefined) => {
    setIsOpen(false);
    button.current?.focus();
    if (option && option.value !== value) {
      onChange(option.value);
    }
  };
  useCloseOutside(container, button, isOpen, () => {
    setIsOpen(false);
  });

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!isOpen) {
        open();
        return;
      }
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((index) => Math.max(0, Math.min(enabled.length - 1, index + step)));
    } else if (isOpen && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      choose(enabled[active]);
    } else if (isOpen && event.key === "Tab") {
      setIsOpen(false);
    }
  };

  return (
    <div
      ref={container}
      data-slot="option-picker"
      className={cn("relative flex-none", isWide && "w-full", className)}
      onKeyDown={onKeyDown}
    >
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={listId}
        aria-label={`${label}: ${typeof current?.label === "string" ? current.label : (current?.value ?? "")}`}
        onClick={() => {
          if (isOpen) {
            setIsOpen(false);
          } else {
            open();
          }
        }}
        className={cn(
          "inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-sm border border-border bg-card pr-2 pl-2.5 text-base whitespace-nowrap text-foreground hover:border-primary-border focus-visible:border-primary focus-visible:shadow-focus focus-visible:outline-none [&>svg]:size-3 [&>svg]:text-muted-foreground [&>svg]:transition-transform",
          isOpen && "border-primary shadow-focus [&>svg]:rotate-180",
          isWide && "h-8 w-full justify-between text-lg",
        )}
      >
        <span>{current?.label}</span>
        <Chevron />
      </button>
      {isOpen && (
        <div
          id={listId}
          role="listbox"
          aria-label={label}
          className={cn(
            "absolute top-[calc(100%+4px)] z-25 w-max max-w-90 min-w-full rounded-md border border-border bg-card p-1 shadow-lg",
            align === "end" ? "right-0" : "left-0",
            isWide && "w-full max-w-none",
          )}
        >
          {options.map((option) => {
            const isChosen = option.value === value;
            const isActive = enabled[active] === option;
            return (
              <button
                key={option.value}
                type="button"
                role="option"
                aria-selected={isChosen}
                disabled={option.disabled}
                onMouseEnter={() => {
                  const index = enabled.indexOf(option);
                  if (index >= 0) {
                    setActive(index);
                  }
                }}
                onClick={() => {
                  choose(option);
                }}
                className={cn(
                  "flex w-full cursor-pointer items-start gap-1.5 rounded-xs py-1.5 pr-2.5 pl-1.5 text-left text-base text-foreground disabled:cursor-default disabled:opacity-50",
                  isActive && "bg-primary-muted",
                )}
              >
                <span className="mt-px size-4 flex-none text-primary [&>svg]:size-4">
                  {isChosen && <Check />}
                </span>
                <span className="grid gap-px">
                  <b className={cn("font-medium", isChosen && "text-primary")}>{option.label}</b>
                  {option.hint && <span className="text-sm text-muted-foreground">{option.hint}</span>}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
