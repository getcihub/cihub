import { type ReactNode, useRef } from "react";

import { useDialog } from "./dialog";
import styles from "./ui.module.css";

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
