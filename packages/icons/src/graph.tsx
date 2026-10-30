import type { ComponentProps } from "react";

export function GraphIcon(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" {...props}>
      <path
        d="M5 8 H7.5 M7.5 8 C7.5 5 8.5 4 10.5 4 M7.5 8 C7.5 11 8.5 12 10.5 12"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <circle cx="3.5" cy="8" r="1.8" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="12.5" cy="4" r="1.8" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="12.5" cy="12" r="1.8" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}
