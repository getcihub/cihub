import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

/** The page waits for the first command on a repository: a beacon, and what moves on once it comes. */
export function FirstCommandWait({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div className={cn("flex items-center gap-2.5 text-left", className)} role="status">
      <span className="mx-0.75 size-2.5 flex-none rounded-full bg-primary motion-safe:animate-beacon" />
      <div>
        <b className="block font-medium">Waiting for the first command</b>
        <span className="text-md text-muted-foreground">{children}</span>
      </div>
    </div>
  );
}
