import { Link, useNavigate } from "@tanstack/react-router";
import { m, useReducedMotion } from "motion/react";
import { type ComponentProps, type ReactNode, useLayoutEffect, useRef, useState } from "react";

import { useDialog } from "@/components/ui/dialog";
import { transitions } from "@/components/ui/motion";
import type { QueuePath } from "@/domain/queue/queries";
import { cn } from "@/lib/cn";

interface Props {
  path: QueuePath;
  title: ReactNode;
  /** Above the title. */
  meta?: ReactNode;
  /** Under the title, in the head. */
  head?: ReactNode;
  children?: ReactNode;
}

// The frames of a drawer on the page. The PR takes the place of its skeleton in one commit: while the skeleton
// still counts, the frame of the PR knows that the drawer is already open, and does not slide in again.
let openFrames = 0;

/**
 * The frame of a drawer over the page of a queue. It slides in from the right; Close, Escape and a click beside
 * it slide it out, then go back to the page, with its tab. The focus goes to the title, and comes back to where
 * it was.
 */
export function DrawerFrame({ path, title, meta, head, children }: Props) {
  const navigate = useNavigate();
  const drawer = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const isCalm = useReducedMotion() ?? false;
  const [isEntering] = useState(() => openFrames === 0);
  const [isClosing, setIsClosing] = useState(false);
  const leave = () => {
    void navigate({ to: "/orgs/$org/repos/$repo/queues/$queue", params: path, search: (kept) => kept });
  };
  // Without movement, nothing slides out: the page comes back at once.
  const close = () => {
    if (isCalm) {
      leave();
      return;
    }
    setIsClosing(true);
  };
  useDialog(drawer, close, { first: heading });

  useLayoutEffect(() => {
    openFrames += 1;
    return () => {
      openFrames -= 1;
    };
  }, []);

  return (
    <>
      <m.div
        className="fixed inset-0 z-30 bg-overlay"
        onClick={close}
        aria-hidden="true"
        initial={isEntering ? { opacity: 0 } : false}
        animate={{ opacity: isClosing ? 0 : 1 }}
        transition={transitions.fade}
      />
      <m.aside
        ref={drawer}
        className="fixed top-0 right-0 bottom-0 z-40 w-125 max-w-screen overflow-x-hidden overflow-y-auto border-l border-border bg-card shadow-lg max-md:w-screen"
        role="dialog"
        aria-modal="true"
        aria-labelledby="pr-title"
        initial={isEntering ? { x: "100%" } : false}
        animate={{ x: isClosing ? "100%" : 0 }}
        transition={transitions.slide}
        onAnimationComplete={() => {
          if (isClosing) {
            leave();
          }
        }}
      >
        <header className="sticky top-0 z-1 border-b border-divider bg-card px-5 py-4">
          <Link
            className="absolute top-4 right-4 text-base text-secondary-foreground hover:text-foreground"
            to="/orgs/$org/repos/$repo/queues/$queue"
            params={path}
            search={(kept) => kept}
            onClick={(event) => {
              // A click that opens the page elsewhere, in a new tab, stays a link.
              if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
                return;
              }
              event.preventDefault();
              close();
            }}
          >
            Close
          </Link>
          {meta && <div className="text-md text-muted-foreground">{meta}</div>}
          <h2
            id="pr-title"
            ref={heading}
            tabIndex={-1}
            className="mt-1 mb-2 text-4xl font-medium focus:outline-none"
          >
            {title}
          </h2>
          {head}
        </header>
        {children}
      </m.aside>
    </>
  );
}

/** A part of the drawer under its head: where the PR is, its run, its history. */
export function DrawerSection({ className, ...props }: ComponentProps<"section">) {
  return <section className={cn("border-b border-divider px-5 py-3.5", className)} {...props} />;
}

export function DrawerSectionTitle({ className, ...props }: ComponentProps<"h3">) {
  return <h3 className={cn("mb-3 text-md font-medium text-secondary-foreground", className)} {...props} />;
}

export function DrawerText({ className, ...props }: ComponentProps<"p">) {
  return <p className={cn("text-base", className)} {...props} />;
}

/** A detail under a part of the drawer, in the color of a hint. */
export function DrawerNote({ className, ...props }: ComponentProps<"p">) {
  return <p className={cn("mt-2.5 text-base text-muted-foreground", className)} {...props} />;
}
