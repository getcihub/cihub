import { Fragment, useRef } from "react";

import { Input } from "@cihub/ui/components/input";

const CODE_LENGTH = 8;

/**
 * The letters of a code, one box each, as `cihub auth login` shows them (`WDJB-MJHT`). A letter moves to the next
 * box, a paste fills the boxes from the one it lands in, and a backspace in an empty box goes back one.
 */
export function CodeBoxes({
  letters,
  isInvalid,
  onChange,
}: {
  letters: string;
  isInvalid: boolean;
  onChange: (letters: string) => void;
}) {
  const boxes = useRef<(HTMLInputElement | null)[]>([]);

  const focus = (index: number) => {
    boxes.current[Math.min(index, CODE_LENGTH - 1)]?.focus();
  };

  const write = (index: number, typed: string) => {
    if (typed === "") {
      onChange(letters.slice(0, index) + letters.slice(index + 1));
      return;
    }

    const added = typed.toUpperCase().replace(/[^A-Z]/g, "");
    if (!added) {
      return;
    }

    // A box after the last letter writes after it: the letters have no gap.
    const start = Math.min(index, letters.length);
    onChange((letters.slice(0, start) + added + letters.slice(start + added.length)).slice(0, CODE_LENGTH));
    focus(start + added.length);
  };

  return (
    <div role="group" aria-label="Code" className="flex items-center gap-1.5">
      {Array.from({ length: CODE_LENGTH }, (_, index) => (
        <Fragment key={index}>
          {index === CODE_LENGTH / 2 && (
            <span aria-hidden className="text-2xl text-muted-foreground">
              -
            </span>
          )}
          <Input
            ref={(box) => {
              boxes.current[index] = box;
            }}
            type="text"
            value={letters[index] ?? ""}
            aria-label={`Letter ${String(index + 1)} of ${String(CODE_LENGTH)}`}
            aria-invalid={isInvalid}
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            className="h-12 w-full min-w-0 text-center font-mono text-3xl font-semibold uppercase"
            onFocus={(event) => {
              // Selected, the letter of the box is replaced by the next one typed.
              event.target.select();
            }}
            onChange={(event) => {
              write(index, event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Backspace" && !letters[index] && index > 0) {
                event.preventDefault();
                onChange(letters.slice(0, index - 1));
                focus(index - 1);
              }
            }}
          />
        </Fragment>
      ))}
    </div>
  );
}
