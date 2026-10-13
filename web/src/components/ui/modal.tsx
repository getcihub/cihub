import { m } from "motion/react";
import { type ComponentProps, type ReactNode, useRef } from "react";

import { cn } from "@/lib/cn";
import { useDialog } from "./hooks/use-dialog";
import { transitions } from "./motion";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/** A modal dialog over the page, and over the drawer of a PR: Escape and a click beside it close it alone. */
export function Modal({ title, onClose, children }: Props) {
  const dialog = useRef<HTMLDivElement>(null);
  useDialog(dialog, onClose, { isOnTop: true });

  return (
    <>
      <m.div
        className="fixed inset-0 z-50 bg-overlay"
        onClick={onClose}
        aria-hidden="true"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={transitions.fade}
      />
      <m.div
        ref={dialog}
        data-slot="modal"
        className="fixed top-[18vh] left-1/2 z-60 grid w-105 max-w-[calc(100vw-32px)] -translate-x-1/2 gap-3.5 rounded-md border border-border bg-card px-5 py-4.5 shadow-lg"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
        initial={{ opacity: 0, y: 4 }}
        animate={{ opacity: 1, y: 0 }}
        transition={transitions.enter}
      >
        <h2 id="modal-title" className="text-2xl font-medium">
          {title}
        </h2>
        {children}
      </m.div>
    </>
  );
}

/** What the action of a modal does. */
export function ModalDescription({ className, ...props }: ComponentProps<"p">) {
  return (
    <p
      data-slot="modal-description"
      className={cn("text-base text-secondary-foreground", className)}
      {...props}
    />
  );
}

/** The buttons of a modal, on the right: the one that keeps things as they are, then the action. */
export function ModalFooter({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="modal-footer" className={cn("flex justify-end gap-2", className)} {...props} />;
}
