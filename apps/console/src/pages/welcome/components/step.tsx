import type { ReactNode } from "react";

import { cn } from "@cihub/ui/lib/cn";

/** A step of the way to the first merge; the one to do now stands out. */
export function Step({
  n,
  title,
  isNow = false,
  children,
}: {
  n: number;
  title: string;
  isNow?: boolean;
  children: ReactNode;
}) {
  return (
    <li
      className={cn(
        "flex gap-3 rounded-md border px-4.5 py-4 text-md",
        isNow ? "border-primary bg-card" : "border-border text-muted-foreground",
      )}
    >
      <span
        className={cn(
          "grid size-6 flex-none place-items-center rounded-full text-sm font-medium",
          isNow ? "bg-primary text-primary-foreground" : "border border-border",
        )}
      >
        {n}
      </span>
      <div className="flex flex-col gap-1">
        <b className={cn("font-medium", isNow && "text-foreground")}>{title}</b>
        {children}
      </div>
    </li>
  );
}
