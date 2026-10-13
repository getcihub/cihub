import { AnimatePresence, m } from "motion/react";
import type { ComponentProps } from "react";

import { transitions } from "@/components/ui/motion";
import { cn } from "@/lib/cn";

/**
 * A message over the corner of the page: it slides in when its caller gives it `children`, and out when the caller
 * takes them back. The caller decides how long it stays.
 */
export function Toast({ className, children, ...props }: ComponentProps<"div">) {
  return (
    <div data-slot="toast" role="status" className={cn("fixed right-5 bottom-5 z-70", className)} {...props}>
      <AnimatePresence>
        {children && (
          <m.p
            key="toast"
            className="rounded-md border border-primary-border bg-card px-3 py-2.25 text-base shadow-lg"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 12 }}
            transition={transitions.slide}
          >
            {children}
          </m.p>
        )}
      </AnimatePresence>
    </div>
  );
}
