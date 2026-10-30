"use client";

import { type VariantProps, cva } from "class-variance-authority";
import { AnimatePresence, m } from "motion/react";
import { type ComponentProps, type ReactNode, useId, useRef, useState } from "react";

import { cn } from "../lib/cn";
import { useCloseOutside } from "../hooks/use-close-outside";
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

  useCloseOutside(container, button, isOpen, () => {
    setIsOpen(false);
  });

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
                : // `w-max`: the button of a right menu can be as narrow as an avatar, and would wrap every entry.
                  // The list passes the edge of the content, into the margin of the page (`px-page`), so that
                  // it does not look like a part of the page under it.
                  "-right-3 w-max max-w-80 min-w-50 max-md:-right-2",
            )}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={transitions.fade}
            onClick={(event) => {
              // A button that opens a list of its own (`MenuSub`) keeps the menu open.
              if (
                event.target instanceof Element &&
                event.target.closest("a, button:not([disabled]):not([aria-expanded])")
              ) {
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

/**
 * An entry of a menu that shows a list of its own at the left of the menu,
 * which aligns right: on a click, or while a mouse rests on it.
 */
export function MenuSub({ label, children }: { label: string; children: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  const listId = useId();

  return (
    <div
      data-slot="menu-sub"
      className="relative"
      // A touch sends `pointerleave` as the finger lifts: only a mouse follows the hover.
      onPointerEnter={(event) => {
        if (event.pointerType === "mouse") {
          setIsOpen(true);
        }
      }}
      onPointerLeave={(event) => {
        if (event.pointerType === "mouse") {
          setIsOpen(false);
        }
      }}
    >
      <MenuItem
        isCurrent={isOpen}
        aria-expanded={isOpen}
        aria-controls={listId}
        // A click only opens: the hover of a mouse opened the list just before it.
        onClick={() => {
          setIsOpen(true);
        }}
      >
        {label}
        <svg
          className="ml-auto size-3.5 text-muted-foreground"
          viewBox="0 0 16 16"
          fill="none"
          aria-hidden="true"
        >
          <path
            d="M6 3.5 L10.5 8 L6 12.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </MenuItem>
      <AnimatePresence>
        {isOpen && (
          // The padding, not a margin, keeps the space between the two lists under the pointer. A narrow
          // window has no room beside the menu: the list opens under its entry.
          <m.div
            key="list"
            className="absolute -top-1.25 right-full z-60 pr-1 max-md:static max-md:pr-0"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={transitions.fade}
          >
            <div
              id={listId}
              role="group"
              aria-label={label}
              className="grid w-max min-w-50 gap-0.5 rounded-md border border-border bg-card p-1 shadow-lg max-md:w-auto max-md:min-w-0 max-md:border-0 max-md:p-0 max-md:pl-3 max-md:shadow-none"
            >
              {children}
            </div>
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

/** A text at the top of a menu or of a group, which the person reads but cannot choose. */
export function MenuLabel({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="menu-label" className={cn("px-2 py-1.5 text-base", className)} {...props} />;
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
