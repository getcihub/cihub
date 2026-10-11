import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { PendingPage } from "./pages/PendingPage";
import { routeTree } from "./routeTree.gen";
import { sendCsrfToken } from "./session";
import "./styles/base.css";

// A read stays fresh for 30 seconds: a click inside an organization reads it
// again only after that. The stream of lot 10d invalidates what changed.
sendCsrfToken();

const queryClient = new QueryClient({ defaultOptions: { queries: { staleTime: 30_000 } } });
const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPendingComponent: PendingPage,
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
        <RouterProvider router={router} />
      </QueryClientProvider>
    </StrictMode>,
  );
}
