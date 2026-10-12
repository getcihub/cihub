import { type VariantProps, cva } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";

/** The classes of a card, for a `<section>` or a `<Link>` that is one. */
export const cardVariants = cva("rounded-md border bg-card", {
  variants: {
    tone: {
      default: "border-border",
      primary: "border-primary-border",
    },
  },
  defaultVariants: {
    tone: "default",
  },
});

export function Card({
  className,
  tone,
  ...props
}: ComponentProps<"div"> & VariantProps<typeof cardVariants>) {
  return <div data-slot="card" className={cn(cardVariants({ tone }), className)} {...props} />;
}

/** The head of a card: its title, then what it shows on the right. */
export function CardHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="card-header"
      className={cn("flex min-h-11 items-center gap-3 border-b border-divider px-4 py-2", className)}
      {...props}
    />
  );
}

export function CardTitle({ className, ...props }: ComponentProps<"h2">) {
  return <h2 data-slot="card-title" className={cn("text-xl font-medium", className)} {...props} />;
}

/** A hint at the right of the head of a card. */
export function CardHint({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="card-hint"
      className={cn("ml-auto text-md text-muted-foreground", className)}
      {...props}
    />
  );
}
