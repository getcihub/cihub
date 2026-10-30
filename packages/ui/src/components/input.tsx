import type { ComponentProps } from "react";

import { cn } from "../lib/cn";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      data-slot="input"
      className={cn(
        "h-7.5 rounded-sm border border-border bg-card px-2 text-foreground placeholder:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}
