import type { ComponentProps } from "react";

/** The logo of CIHub, in the blue of the theme. */
export function Mark(props: ComponentProps<"svg">) {
  return (
    <svg viewBox="0 0 56 56" aria-hidden="true" {...props}>
      <defs>
        {/* Each mark repeats the same gradient: an identical definition under the same id. */}
        <linearGradient id="cihub-mark" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: "var(--primary-top)" }} />
          <stop offset="1" style={{ stopColor: "var(--primary)" }} />
        </linearGradient>
      </defs>
      <rect width="56" height="56" rx="14" fill="url(#cihub-mark)" />
      <path
        d="M20 16 V40 M36 18 Q36 32 20 34"
        stroke="#fff"
        strokeWidth="3.5"
        strokeLinecap="round"
        fill="none"
      />
      <circle cx="20" cy="15" r="4" fill="#fff" />
      <circle cx="20" cy="41" r="4" fill="#fff" />
      <circle cx="36" cy="17" r="4" fill="#fff" />
    </svg>
  );
}
