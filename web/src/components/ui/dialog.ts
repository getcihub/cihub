import { type RefObject, useEffect, useEffectEvent } from "react";

const FOCUSABLE =
  "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex='0']";

/**
 * The keyboard of a dialog over the page: the focus goes to `first`, else to its first control, stays inside with
 * Tab, and comes back to where it was once the dialog closes; Escape calls `onClose`. A modal over a drawer takes
 * Escape first (`isOnTop`), so that it closes alone.
 */
export function useDialog(
  container: RefObject<HTMLElement | null>,
  onClose: () => void,
  { isOnTop = false, first }: { isOnTop?: boolean; first?: RefObject<HTMLElement | null> } = {},
): void {
  const close = useEffectEvent(onClose);

  useEffect(() => {
    const before = document.activeElement;
    const controls = () => [...(container.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])];
    (first?.current ?? controls()[0])?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        close();
        return;
      }
      if (event.key === "Tab") {
        keepFocusInside(event, controls());
      }
    };
    window.addEventListener("keydown", onKey, isOnTop);

    return () => {
      window.removeEventListener("keydown", onKey, isOnTop);
      if (before instanceof HTMLElement && before.isConnected) {
        before.focus();
      }
    };
  }, [container, first, isOnTop]);
}

/** Tab from the last control goes to the first one, and Shift+Tab from the first one to the last. */
function keepFocusInside(event: KeyboardEvent, controls: HTMLElement[]): void {
  const edge = event.shiftKey ? controls[0] : controls.at(-1);
  const other = event.shiftKey ? controls.at(-1) : controls[0];

  const focused = document.activeElement;

  if (focused === edge || !controls.some((control) => control === focused)) {
    event.preventDefault();
    other?.focus();
  }
}
