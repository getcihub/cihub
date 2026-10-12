import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { createQueryClient } from "@/lib/queryClient";
import { MotionProvider } from "@/ui/motion";
import { routeTree } from "./routeTree.gen";
import { sendCsrfToken } from "@/domain/session/session";
import "@/styles/globals.css";

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
