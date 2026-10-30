import type { ReactNode } from "react";

import { ErrorIcon } from "@cihub/icons/error";
import { Alert } from "@cihub/ui/components/alert";
import { cn } from "@cihub/ui/lib/cn";

/** What failed, in red, with the icon on the first line. */
export function ErrorAlert({ className, children }: { className?: string; children: ReactNode }) {
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
