import { type VariantProps, cva } from "class-variance-authority";
import { AnimatePresence, m } from "motion/react";
import { type ComponentProps, type ReactNode, useEffect, useId, useRef, useState } from "react";

import { cn } from "@/lib/cn";
import { transitions } from "./motion";

interface Props {
  /** What the list offers, as a screen reader names the button: "Organization". */
  label: string;
  /** What the button shows: the current choice. */
  trigger: ReactNode;
  triggerClassName: string;
  align: "left" | "right";
  children: ReactNode;
}

/**
 * A button that shows a list of links and buttons. A choice, `Escape`, a click
 * outside or the focus that leaves it closes the list; `Escape` gives the
 * focus back to the button.
 */
export function Menu({ label, trigger, triggerClassName, align, children }: Props) {
  const [isOpen, setIsOpen] = useState(false);
  const container = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const listId = useId();

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const closeOutside = (event: MouseEvent) => {
      if (!container.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
        button.current?.focus();
      }
    };
    document.addEventListener("mousedown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [isOpen]);

  return (
    <div
      data-slot="menu"
      className="relative min-w-0"
      ref={container}
      onBlur={(event) => {
        if (!container.current?.contains(event.relatedTarget)) {
          setIsOpen(false);
        }
      }}
    >
      <button
        type="button"
        ref={button}
        className={triggerClassName}
        aria-expanded={isOpen}
        aria-controls={listId}
        onClick={() => {
          setIsOpen((wasOpen) => !wasOpen);
        }}
      >
        <span className="sr-only">{label}:</span> {trigger}
      </button>
      <AnimatePresence>
        {isOpen && (
          <m.div
            key="list"
            id={listId}
            role="group"
            aria-label={label}
            className={cn(
              "absolute top-[calc(100%_+_6px)] z-60 grid min-w-65 gap-0.5 rounded-md border border-border bg-card p-1 font-normal shadow-lg",
              align === "left"
                ? "left-0 max-md:fixed max-md:top-14 max-md:left-3 max-md:w-[calc(100vw_-_24px)] max-md:min-w-0"
                : "right-0 min-w-50",
            )}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={transitions.fade}
            onClick={(event) => {
              if ((event.target as HTMLElement).closest("a, button:not([disabled])")) {
                setIsOpen(false);
              }
            }}
          >
            {children}
          </m.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/** The classes of an entry of a menu, for a `<Link>`, an `<a>` or a `<button>`. */
export const menuItemVariants = cva(
  "flex w-full cursor-pointer items-center gap-2.5 rounded-sm px-2 py-1.5 text-left text-base text-foreground hover:bg-muted disabled:cursor-default disabled:text-muted-foreground disabled:hover:bg-transparent",
  {
    variants: {
      isCurrent: {
        true: "bg-muted",
        false: "",
      },
    },
    defaultVariants: {
      isCurrent: false,
    },
  },
);

export function MenuItem({
  className,
  isCurrent,
  type = "button",
  ...props
}: ComponentProps<"button"> & VariantProps<typeof menuItemVariants>) {
  return (
    <button
      data-slot="menu-item"
      type={type}
      className={cn(menuItemVariants({ isCurrent }), className)}
      {...props}
    />
  );
}

export function MenuSeparator({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="menu-separator" className={cn("my-1 h-px bg-divider", className)} {...props} />;
}

/** A detail at the right of an entry: a role, a count. */
export function MenuHint({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      data-slot="menu-hint"
      className={cn("ml-auto text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}
