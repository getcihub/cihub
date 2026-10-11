import { type RefObject, useLayoutEffect, useRef } from "react";

const SLIDE_MS = 300;

/**
 * Slides each element of `container` marked `data-flip` from its last place to its new one when the data of the
 * page moves it: what changed moves, as in the mockups v5. Nothing moves for a person who asks for less motion.
 */
export function useFlip(container: RefObject<HTMLElement | null>): void {
  const places = useRef(new Map<string, { left: number; top: number }>());

  useLayoutEffect(() => {
    const root = container.current;
    if (!root) {
      return;
    }

    // A browser without media queries, as the tests have, gets no motion.
    const media = (window as Partial<Window>).matchMedia;
    const isCalm =
      typeof media !== "function" || media.call(window, "(prefers-reduced-motion: reduce)").matches;
    // A place in the container, so that a scroll of the page moves nothing.
    const origin = root.getBoundingClientRect();
    const next = new Map<string, { left: number; top: number }>();

    for (const element of root.querySelectorAll<HTMLElement>("[data-flip]")) {
      const box = element.getBoundingClientRect();
      const place = { left: box.left - origin.left, top: box.top - origin.top };
      const before = places.current.get(element.dataset.flip ?? "");
      next.set(element.dataset.flip ?? "", place);
      if (before && !isCalm) {
        slide(element, before.left - place.left, before.top - place.top);
      }
    }

    places.current = next;
  });
}

function slide(element: HTMLElement, dx: number, dy: number): void {
  if (Math.abs(dx) < 1 && Math.abs(dy) < 1) {
    return;
  }

  element.animate([{ transform: `translate(${String(dx)}px, ${String(dy)}px)` }, { transform: "none" }], {
    duration: SLIDE_MS,
    easing: "ease-out",
  });
}
