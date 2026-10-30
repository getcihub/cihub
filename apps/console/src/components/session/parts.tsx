import type { ReactNode } from "react";

import { Mark } from "@cihub/icons/brand/mark";
import { ErrorIcon } from "@cihub/icons/error";
import { Alert } from "@cihub/ui/components/alert";
import { cn } from "@cihub/ui/lib/cn";

export function Brand({ className }: { className?: string }) {
  return (
    <div className={cn("flex items-center gap-2.5 text-3xl font-semibold tracking-[-0.3px]", className)}>
      <Mark className="size-6 flex-none" />
      CIHub
    </div>
  );
}

export function Title({ children }: { children: ReactNode }) {
  return <h1 className="mb-2 text-5xl font-semibold tracking-[-0.5px]">{children}</h1>;
}

export function Lead({ children }: { children: ReactNode }) {
  return <p className="mb-7 text-lg text-secondary-foreground">{children}</p>;
}

export function Note({ children }: { children: ReactNode }) {
  return <p className="mt-4 text-md text-muted-foreground">{children}</p>;
}

/** What failed, in red, with the icon on the first line. */
export function Failure({ className, children }: { className: string; children: ReactNode }) {
  return (
    <Alert
      variant="destructive"
      className={cn(
        "items-start py-2.5 font-medium text-destructive [&>svg]:mt-px [&>svg]:size-4.5",
        className,
      )}
    >
      <ErrorIcon />
      <span>{children}</span>
    </Alert>
  );
}
