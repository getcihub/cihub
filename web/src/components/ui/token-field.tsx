import { type KeyboardEvent, type ReactNode, useState } from "react";

import { cn } from "@/lib/cn";

/** A value that the field offers, and what the menu says of it. */
export interface Suggestion {
  value: string;
  meta?: string;
}

/**
 * Values that the person types one by one, each a token: Enter or a comma adds one, Backspace in the empty field
 * removes the last one, × removes any. While the field has the focus, a menu offers the `suggestions` that match
 * the text, then the text itself unless the values are `isStrict`; the arrows move in it. Read only, the tokens
 * stand alone, without the field.
 */
export function TokenField({
  label,
  values,
  placeholder,
  onChange,
  suggestions = [],
  isStrict = false,
  note,
  readOnly = false,
  isIncomplete = false,
  className,
}: {
  label: string;
  values: string[];
  placeholder: string;
  onChange: (values: string[]) => void;
  suggestions?: Suggestion[];
  /** Only the suggestions are values. */
  isStrict?: boolean;
  /** What the menu says under the suggestions: where they come from, or how to type a value. */
  note?: ReactNode;
  readOnly?: boolean;
  /** No value yet: the border says that the condition waits for one. */
  isIncomplete?: boolean;
  className?: string;
}) {
  const [text, setText] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const [active, setActive] = useState(0);
  const query = text.trim();
  const matching = suggestions
    .filter((item) => !values.includes(item.value) && item.value.toLowerCase().includes(query.toLowerCase()))
    .slice(0, 8);
  const isNew =
    query !== "" && !isStrict && !values.includes(query) && !suggestions.some((item) => item.value === query);
  const items: (Suggestion & { isNew?: boolean })[] = isNew
    ? [...matching, { value: query, meta: "not seen yet", isNew: true }]
    : matching;

  const add = (value: string) => {
    if (value && !values.includes(value) && (!isStrict || suggestions.some((item) => item.value === value))) {
      onChange([...values, value]);
    }
    setText("");
    setActive(0);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setIsOpen(true);
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((index) => Math.max(0, Math.min(items.length - 1, index + step)));
    } else if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      add(items[active]?.value ?? query);
    } else if (event.key === "Escape") {
      setIsOpen(false);
    } else if (event.key === "Backspace" && text === "" && values.length > 0) {
      onChange(values.slice(0, -1));
    }
  };

  return (
    <div
      role="group"
      aria-label={label}
      data-slot="token-field"
      className={cn(
        "relative flex min-h-7 flex-wrap items-center gap-1",
        !readOnly &&
          "max-w-140 min-w-55 flex-1 rounded-sm border border-border bg-card px-1 py-0.5 focus-within:border-primary focus-within:shadow-focus max-md:min-w-0 max-md:basis-full",
        !readOnly && isIncomplete && "border-dashed border-warning-border",
        className,
      )}
    >
      {values.map((value) => (
        <span
          key={value}
          data-slot="token"
          className={cn(
            "inline-flex h-5.5 items-center gap-0.5 rounded-xs border border-border bg-muted pl-2 font-mono text-md whitespace-nowrap",
            readOnly ? "pr-2" : "pr-0.5",
          )}
        >
          {value}
          {!readOnly && (
            <button
              type="button"
              className="size-4.5 cursor-pointer text-base leading-none text-muted-foreground hover:text-destructive"
              aria-label={`Remove ${value}`}
              onClick={() => {
                onChange(values.filter((other) => other !== value));
              }}
            >
              ×
            </button>
          )}
        </span>
      ))}
      {!readOnly && (
        <input
          className="h-5.5 min-w-20 flex-1 bg-transparent px-1 text-base outline-none"
          role="combobox"
          aria-expanded={isOpen}
          aria-label={placeholder}
          autoComplete="off"
          placeholder={values.length === 0 ? placeholder : undefined}
          value={text}
          onFocus={() => {
            setIsOpen(true);
          }}
          onChange={(event) => {
            setText(event.target.value);
            setActive(0);
            setIsOpen(true);
          }}
          onKeyDown={onKeyDown}
          onBlur={() => {
            setIsOpen(false);
            if (!isStrict) {
              add(query);
            }
            setText("");
          }}
        />
      )}
      {!readOnly && isOpen && (items.length > 0 || note) && (
        <div
          role="listbox"
          aria-label={label}
          className="absolute top-[calc(100%+4px)] -left-px z-20 w-[max(100%,360px)] rounded-md border border-border bg-card p-1 shadow-lg"
        >
          {items.map((item, index) => (
            <button
              key={item.value}
              type="button"
              role="option"
              aria-selected={index === active}
              // The field keeps the focus: a click chooses before the blur closes the menu.
              onMouseDown={(event) => {
                event.preventDefault();
                add(item.value);
              }}
              className={cn(
                "flex w-full cursor-pointer items-baseline justify-between gap-3 rounded-xs px-2 py-1.5 text-left text-base hover:bg-primary-muted",
                index === active && "bg-primary-muted",
              )}
            >
              <span className="font-mono text-md">{item.isNew ? `Add “${item.value}”` : item.value}</span>
              {item.meta && (
                <span className="text-sm whitespace-nowrap text-muted-foreground">{item.meta}</span>
              )}
            </button>
          ))}
          {items.length === 0 && (
            <div className="p-2 text-md text-muted-foreground">
              {query ? "Nothing matches." : "Every value is chosen."}
            </div>
          )}
          {note && (
            <div className="mt-1 border-t border-divider px-2 pt-1.5 pb-0.5 text-xs text-muted-foreground">
              {note}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
