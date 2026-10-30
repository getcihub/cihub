import type { ComponentProps } from "react";

export function ErrorIcon(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 20 20" fill="none" aria-hidden="true" {...props}>
      <circle cx="10" cy="10" r="8" stroke="currentColor" strokeWidth="1.8" />
      <path d="M10 6 V11 M10 14 V14.2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}
