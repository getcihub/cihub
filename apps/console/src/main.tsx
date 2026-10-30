import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { MotionProvider } from "@cihub/ui/components/motion";
import { sendCsrfToken } from "@/domain/session/session";
import { createQueryClient } from "@/lib/query-client";
import "@/styles/globals.css";
import { routeTree } from "./routeTree.gen";

sendCsrfToken();

const queryClient = createQueryClient();
const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPendingMs: 200,
  defaultPendingMinMs: 500,
  defaultPreload: "intent",
  defaultPreloadStaleTime: 0,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <MotionProvider>
          <RouterProvider router={router} />
        </MotionProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
}
