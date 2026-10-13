import { type VariantProps, cva } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";
import { Spinner } from "./spinner";

/** The classes of a button, for a `<Link>` or an `<a>` that looks like one. */
export const buttonVariants = cva(
  "inline-flex cursor-pointer items-center gap-1.5 rounded-sm border px-2.5 font-medium whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-60 aria-disabled:pointer-events-none aria-disabled:opacity-60",
  {
    variants: {
      variant: {
        default: "border-border bg-card text-foreground hover:border-muted-foreground",
        primary:
          "border-primary bg-primary text-primary-foreground hover:border-primary-hover hover:bg-primary-hover",
        destructive: "border-destructive bg-destructive text-primary-foreground",
        "destructive-outline": "border-border bg-card text-destructive hover:border-muted-foreground",
      },
      size: {
        default: "h-7 text-base [&>svg]:size-3.5",
        lg: "h-9 w-full justify-center text-xl [&_svg]:size-3.5",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

interface PendingProps {
  /** The action runs: the button turns, says `pendingText`, and waits. */
  isPending?: boolean;
  /** What the button says while the action runs: "Submitting". */
  pendingText?: string;
}

export function Button({
  className,
  variant = "default",
  size = "default",
  type = "button",
  isPending = false,
  pendingText,
  disabled,
  children,
  ...props
}: ComponentProps<"button"> & VariantProps<typeof buttonVariants> & PendingProps) {
  return (
    <button
      data-slot="button"
      data-variant={variant}
      data-size={size}
      type={type}
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled === true || isPending}
      aria-busy={isPending || undefined}
      {...props}
    >
      {isPending ? (
        <>
          <Spinner />
          {pendingText ?? children}
        </>
      ) : (
        children
      )}
    </button>
  );
}
