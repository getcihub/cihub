import type { ComponentProps } from "react";

import { cn } from "../lib/cn";

/** The head of a part of a page, above its card: a `SectionTitle`, then what it shows on the right. */
export function SectionHead({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="section-head"
      className={cn("-mb-1 flex min-h-7 flex-wrap items-center gap-3", className)}
      {...props}
    />
  );
}

export function SectionTitle({ className, ...props }: ComponentProps<"h2">) {
  return <h2 data-slot="section-title" className={cn("text-xl font-medium", className)} {...props} />;
}

/** How many entries the part holds, beside its title. */
export function SectionCount({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      data-slot="section-count"
      className={cn("ml-1.5 text-md font-normal text-muted-foreground tabular-nums", className)}
      {...props}
    />
  );
}
