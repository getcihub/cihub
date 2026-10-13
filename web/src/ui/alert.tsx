import { type VariantProps, cva } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";

const alertVariants = cva(
  "flex items-center gap-2.5 rounded-sm border px-3 py-2.25 text-base [&>svg]:size-4 [&>svg]:flex-none",
  {
    variants: {
      variant: {
        info: "border-info-border bg-info-muted [&>svg]:text-info",
        warning: "border-warning-border bg-warning-muted [&>svg]:text-warning",
        destructive: "border-destructive-border bg-destructive-muted [&>svg]:text-destructive",
      },
    },
    defaultVariants: {
      variant: "info",
    },
  },
);

/** A message in the color of its tone: an icon first, then an `AlertContent`. */
export function Alert({
  className,
  variant = "info",
  ...props
}: ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
  return (
    <div
      data-slot="alert"
      data-variant={variant}
      role="alert"
      className={cn(alertVariants({ variant }), className)}
      {...props}
    />
  );
}

/** The text of an alert, beside its icon: a title, then a description on the same line. */
export function AlertContent(props: ComponentProps<"div">) {
  return <div data-slot="alert-content" {...props} />;
}

export function AlertTitle({ className, ...props }: ComponentProps<"span">) {
  return <span data-slot="alert-title" className={cn("font-medium", className)} {...props} />;
}

export function AlertDescription({ className, ...props }: ComponentProps<"p">) {
  return (
    <p
      data-slot="alert-description"
      className={cn("m-0 ml-1 inline text-secondary-foreground", className)}
      {...props}
    />
  );
}
