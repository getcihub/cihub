import { type ReactNode, useEffect, useId, useRef, useState } from "react";

import ui from "../ui/ui.module.css";
import styles from "./Frame.module.css";

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
      className={styles.menuAnchor}
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
        <span className={ui.hidden}>{label}:</span> {trigger}
      </button>
      {isOpen && (
        <div
          id={listId}
          role="group"
          aria-label={label}
          className={`${styles.menu} ${align === "left" ? styles.left : styles.right}`}
          onClick={(event) => {
            if ((event.target as HTMLElement).closest("a, button:not([disabled])")) {
              setIsOpen(false);
            }
          }}
        >
          {children}
        </div>
      )}
    </div>
  );
}
