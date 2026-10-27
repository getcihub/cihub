import { type VariantProps, cva } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "./cn";

const alertVariants = cva(
  "flex items-center gap-2.5 rounded-sm border px-3 py-2.25 text-base [&>svg]:size-4 [&>svg]:flex-none",
  {
    variants: {
      variant: {
        info: "[&>svg]:text-info",
        warning: "[&>svg]:text-warning",
        destructive: "[&>svg]:text-destructive",
      },
      isBanner: {
        false: "",
        true: "border-border bg-card",
      },
    },
    compoundVariants: [
      { variant: "info", isBanner: false, className: "border-info-border bg-info-muted" },
      { variant: "warning", isBanner: false, className: "border-warning-border bg-warning-muted" },
      {
        variant: "destructive",
        isBanner: false,
        className: "border-destructive-border bg-destructive-muted",
      },
      { variant: "info", isBanner: true, className: "shadow-[inset_2px_0_0_var(--color-info)]" },
      { variant: "warning", isBanner: true, className: "shadow-[inset_2px_0_0_var(--color-warning)]" },
      {
        variant: "destructive",
        isBanner: true,
        className: "shadow-[inset_2px_0_0_var(--color-destructive)]",
      },
    ],
    defaultVariants: {
      variant: "info",
      isBanner: false,
    },
  },
);

/**
 * A message in the color of its tone: an icon first, then an `AlertContent`. A banner, the state of a page that
 * stays while the page is open, keeps a neutral surface and shows its tone in its icon and a rule on its left.
 */
export function Alert({
  className,
  variant = "info",
  isBanner = false,
  ...props
}: ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
  return (
    <div
      data-slot="alert"
      data-variant={variant}
      role="alert"
      className={cn(alertVariants({ variant, isBanner }), className)}
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
