import { cn } from "./cn";

/** A ring that turns while an action runs, in the color of its text. */
export function Spinner({ className }: { className?: string }) {
  return (
    <span
      data-slot="spinner"
      aria-hidden="true"
      className={cn(
        "inline-block size-3.5 flex-none animate-spin rounded-full border-2 border-current/30 border-t-current",
        className,
      )}
    />
  );
}
