import { RouterProvider, createMemoryHistory, createRootRoute, createRouter } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import type { ReactNode } from "react";

import { MotionProvider } from "@cihub/ui/components/motion";

/** Renders `ui` in a router of its own, so that its links work. */
export function renderInRouter(ui: ReactNode) {
  const root = createRootRoute({ component: () => ui });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(
    <MotionProvider skipAnimations>
      <RouterProvider router={router} />
    </MotionProvider>,
  );
}
