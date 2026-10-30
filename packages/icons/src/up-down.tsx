import type { ComponentProps } from "react";

/** Up and down: a picker opens a list. */
export function UpDownIcon(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true" {...props}>
      <path
        d="M5 6 L8 3 L11 6 M5 10 L8 13 L11 10"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
