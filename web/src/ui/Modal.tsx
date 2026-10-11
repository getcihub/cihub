import { type ReactNode, useEffect, useRef } from "react";

import styles from "./ui.module.css";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/**
 * A modal dialog over the page: the focus goes to its first field or button, and comes back to where it was
 * once it closes; `Escape` and a click beside it close it.
 */
export function Modal({ title, onClose, children }: Props) {
  const dialog = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const before = document.activeElement;
    dialog.current?.querySelector<HTMLElement>("input, select, button")?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      if (before instanceof HTMLElement) {
        before.focus();
      }
    };
  }, [onClose]);

  return (
    <>
      <div className={styles.scrim} onClick={onClose} aria-hidden="true" />
      <div
        ref={dialog}
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
      >
        <h2 id="modal-title">{title}</h2>
        {children}
      </div>
    </>
  );
}
