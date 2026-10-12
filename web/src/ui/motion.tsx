import { LazyMotion, MotionConfig, type Transition, domAnimation, useReducedMotion } from "motion/react";
import { type ReactNode, type RefObject, useLayoutEffect, useRef } from "react";

// The timings of the mockups v5: the `ease` of CSS, from 120 ms for a menu to 200 ms for the drawer.
const EASE = [0.25, 0.1, 0.25, 1] as const;

export const transitions = {
  /** A menu, the scrim of a modal. */
  fade: { duration: 0.12, ease: EASE },
  /** A page that opens, a modal. */
  enter: { duration: 0.18, ease: EASE },
  /** The drawer of a PR, a notice. */
  slide: { duration: 0.2, ease: EASE },
} satisfies Record<string, Transition>;

/**
 * The animations of the dashboard: only the features that it uses (`m.*` components, `domAnimation`), loaded with
 * the first page, as a page or a drawer starts hidden until they animate it. A person who asks for less motion
 * gets none, fades included: every value is set at once, as `skipAnimations` does for the tests.
 */
export function MotionProvider({
  children,
  skipAnimations = false,
}: {
  children: ReactNode;
  skipAnimations?: boolean;
}) {
  const isCalm = useReducedMotion() ?? false;

  return (
    <LazyMotion features={domAnimation} strict>
      <MotionConfig reducedMotion="user" skipAnimations={skipAnimations || isCalm}>
        {children}
      </MotionConfig>
    </LazyMotion>
  );
}

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

    const isCalm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
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
