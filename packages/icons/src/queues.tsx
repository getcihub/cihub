import type { ComponentProps } from "react";

export function QueuesIcon(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 20 20" fill="none" aria-hidden="true" {...props}>
      <path d="M4 5 H16 M4 10 H16 M4 15 H11" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  );
}
