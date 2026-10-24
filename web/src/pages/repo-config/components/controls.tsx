import { useState } from "react";

import { Input } from "@/components/ui/input";
import { OptionPicker, type PickerOption } from "@/components/ui/option-picker";
import { cn } from "@/lib/cn";

/**
 * An integer setting: the person types it, and the builder takes it when the field loses the focus or on Enter.
 * The server checks its range; a text that is not an integer goes back to the value.
 */
export function NumberField({
  value,
  unit,
  label,
  disabled,
  onCommit,
}: {
  value: number;
  unit?: string;
  label: string;
  disabled: boolean;
  onCommit: (value: number) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text !== null && /^\d+$/.test(text) && Number(text) !== value) {
      onCommit(Number(text));
    }
    setText(null);
  };

  return (
    <span className="inline-flex items-center gap-1.5">
      <Input
        className="w-16 text-right tabular-nums"
        inputMode="numeric"
        aria-label={label}
        disabled={disabled}
        value={text ?? String(value)}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          }
        }}
      />
      {unit && <span className="text-md text-muted-foreground">{unit}</span>}
    </span>
  );
}

/** A text setting, taken when the field loses the focus or on Enter. */
export function TextField({
  value,
  label,
  disabled,
  onCommit,
  className,
}: {
  value: string;
  label: string;
  disabled: boolean;
  onCommit: (value: string) => void;
  className?: string;
}) {
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text !== null && text !== value) {
      onCommit(text);
    }
    setText(null);
  };

  return (
    <Input
      className={cn("w-56", className)}
      aria-label={label}
      disabled={disabled}
      value={text ?? value}
      onChange={(event) => {
        setText(event.target.value);
      }}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          commit();
        }
      }}
    />
  );
}

/**
 * One value among a closed set, in the words of the builder: a picker with a description of each option, or,
 * read only, the words of the value.
 */
export function ChoiceField<T extends string>({
  value,
  options,
  label,
  disabled,
  onChange,
}: {
  value: T;
  options: PickerOption<T>[];
  label: string;
  disabled: boolean;
  onChange: (value: T) => void;
}) {
  if (disabled) {
    return (
      <span className="text-md text-muted-foreground">
        {options.find((option) => option.value === value)?.label ?? value}
      </span>
    );
  }

  return <OptionPicker label={label} value={value} options={options} onChange={onChange} align="end" />;
}

/**
 * An integer that a part may leave to a weaker one: an empty field takes the value of `inherited`, shown in grey.
 */
export function InheritedNumberField({
  value,
  inherited,
  unit,
  label,
  disabled,
  onCommit,
}: {
  value: number | null;
  inherited: number;
  unit?: string;
  label: string;
  disabled: boolean;
  onCommit: (value: number | null) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text !== null && (text === "" || /^\d+$/.test(text))) {
      const next = text === "" ? null : Number(text);
      if (next !== value) {
        onCommit(next);
      }
    }
    setText(null);
  };

  return (
    <span className="inline-flex items-center gap-1.5">
      <Input
        className="w-16 text-right tabular-nums"
        inputMode="numeric"
        aria-label={label}
        disabled={disabled}
        placeholder={String(inherited)}
        value={text ?? (value === null ? "" : String(value))}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          }
        }}
      />
      {unit && <span className="text-md text-muted-foreground">{unit}</span>}
    </span>
  );
}
