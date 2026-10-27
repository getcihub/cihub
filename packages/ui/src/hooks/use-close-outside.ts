"use client";

import { type RefObject, useEffect, useEffectEvent } from "react";

/**
 * Closes what `container` holds over the page while `isOpen`: a mouse down outside it calls `onClose`; `Escape`
 * calls it too, and gives the focus back to `trigger`.
 */
export function useCloseOutside(
  container: RefObject<HTMLElement | null>,
  trigger: RefObject<HTMLElement | null>,
  isOpen: boolean,
  onClose: () => void,
): void {
  const close = useEffectEvent(onClose);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const closeOutside = (event: MouseEvent) => {
      if (!(event.target instanceof Node && container.current?.contains(event.target))) {
        close();
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
        trigger.current?.focus();
      }
    };
    document.addEventListener("mousedown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [container, trigger, isOpen]);
}
