/** Up and down: a picker opens a list. */
export function UpDownIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
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
