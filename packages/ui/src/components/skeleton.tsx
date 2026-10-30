import type { ComponentProps } from "react";

import { cn } from "../lib/cn";

/**
 * The shape of a part of a page while it loads: a line by default, a block with a height. A screen reader skips
 * it; the page that loads says "Loading" once.
 */
export function Skeleton({ className, ...props }: ComponentProps<"span">) {
  return (
    // A span, so that a skeleton can stand for the text of a heading.
    <span
      data-slot="skeleton"
      aria-hidden="true"
      className={cn("block h-3 animate-shimmer rounded-xs bg-shimmer motion-reduce:animate-none", className)}
      {...props}
    />
  );
}

/** What a screen reader says once for a page that loads, instead of its skeletons. */
export function LoadingStatus() {
  return (
    <span className="sr-only" role="status">
      Loading
    </span>
  );
}
