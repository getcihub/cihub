"use client";

import { m } from "motion/react";
import type { ComponentProps } from "react";

import { cn } from "../lib/cn";
import { transitions } from "./motion";

/** The content of a page under the top bar, centered at the width of the mockups; it fades in as it opens. */
export function Page({ className, ...props }: ComponentProps<typeof m.main>) {
  return (
    <m.main
      data-slot="page"
      className={cn(
        "mx-auto grid w-full max-w-page grid-cols-[minmax(0,1fr)] gap-4 px-6 py-5 max-md:gap-3 max-md:p-3.5",
        className,
      )}
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transitions.enter}
      {...props}
    />
  );
}

/** The head of a page: where it is and its title, then its actions on the right. */
export function PageHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div data-slot="page-header" className={cn("flex flex-wrap items-end gap-4", className)} {...props} />
  );
}

/** Where the page is, above its title: "acme/web". */
export function PageLocation({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="page-location"
      className={cn("text-md font-medium text-muted-foreground", className)}
      {...props}
    />
  );
}

export function PageTitle({ className, ...props }: ComponentProps<"h1">) {
  return (
    <h1
      data-slot="page-title"
      className={cn("mt-0.5 text-5xl font-semibold tracking-[-0.3px]", className)}
      {...props}
    />
  );
}
