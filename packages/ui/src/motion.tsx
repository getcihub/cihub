"use client";

import { LazyMotion, MotionConfig, type Transition, domAnimation, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";

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
