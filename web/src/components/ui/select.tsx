import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";

export function Select({ className, ...props }: ComponentProps<"select">) {
  return (
    <select
      data-slot="select"
      className={cn("h-7.5 rounded-sm border border-border bg-card px-2 text-foreground", className)}
      {...props}
    />
  );
}
