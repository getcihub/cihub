import { type VariantProps, cva } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "./cn";

const badgeDotVariants = cva("rounded-full", {
  variants: {
    tone: {
      info: "bg-primary",
      success: "bg-success",
      warning: "bg-warning",
      destructive: "bg-destructive",
      neutral: "bg-waiting",
    },
    isRing: {
      false: "size-1.75",
      true: "size-2 border-[1.5px] border-waiting bg-transparent",
    },
  },
  compoundVariants: [{ tone: "info", isRing: true, className: "border-primary" }],
  defaultVariants: {
    tone: "neutral",
    isRing: false,
  },
});

/** A state as a dot and a word, in the color of its tone; a ring is a state that waits. */
export function Badge({
  className,
  tone = "neutral",
  isRing = false,
  children,
  ...props
}: ComponentProps<"span"> & VariantProps<typeof badgeDotVariants>) {
  return (
    <span
      data-slot="badge"
      data-variant={tone}
      className={cn(
        "inline-flex items-center gap-1.75 text-base font-medium whitespace-nowrap text-foreground",
        className,
      )}
      {...props}
    >
      <span data-slot="badge-dot" className={cn(badgeDotVariants({ tone, isRing }))} />
      {children}
    </span>
  );
}
