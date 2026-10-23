import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";

/** A setting that is on or off: a button with the role `switch`. Disabled, it only shows the value. */
export function Switch({
  checked,
  className,
  ...props
}: Omit<ComponentProps<"button">, "role" | "type"> & { checked: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      data-slot="switch"
      data-variant={checked ? "on" : "off"}
      className={cn(
        "flex h-5 w-8.5 flex-none cursor-pointer rounded-full p-0.5 disabled:cursor-default disabled:opacity-60",
        checked ? "bg-primary" : "bg-border",
        className,
      )}
      {...props}
    >
      <span
        data-slot="switch-thumb"
        className={cn(
          "size-4 rounded-full bg-primary-foreground transition-transform",
          checked && "translate-x-3.5",
        )}
      />
    </button>
  );
}
