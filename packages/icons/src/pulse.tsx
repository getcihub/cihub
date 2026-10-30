import type { ComponentProps } from "react";

export function PulseIcon(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" {...props}>
      <path
        d="M1.5 8.5 H4.5 L6.5 3.5 L9.5 12.5 L11.5 8.5 H14.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
