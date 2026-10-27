export function CopyIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="5" y="5" width="8" height="8" rx="2" stroke="currentColor" strokeWidth="1.5" />
      <path d="M3 10 V4 C3 3.4 3.4 3 4 3 H10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}
